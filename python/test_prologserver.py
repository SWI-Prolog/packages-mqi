import gc
import json
import logging
import os
from datetime import time
from tempfile import gettempdir, mkdtemp
import sys
import unittest
import threading
from time import sleep, perf_counter
from unittest import TestSuite
from swiplserver import *
from pathlib import PurePath, PurePosixPath, PureWindowsPath, Path
import subprocess
from contextlib import suppress
import tempfile
import traceback


# From: https://eli.thegreenplace.net/2011/08/02/python-unit-testing-parametrized-test-cases/
class ParametrizedTestCase(unittest.TestCase):
    """TestCase classes that want to be parametrized should
    inherit from this class.
    """

    def __init__(
        self,
        methodName="runTest",
        essentialOnly=False,
        failOnUnlikely=False,
        launchServer=True,
        useUnixDomainSocket=None,
        serverPort=None,
        password=None,
    ):
        super(ParametrizedTestCase, self).__init__(methodName)
        self.launchServer = launchServer
        self.useUnixDomainSocket = useUnixDomainSocket
        self.serverPort = serverPort
        self.password = password
        self.essentialOnly = essentialOnly
        self.failOnUnlikely = failOnUnlikely
        self.prologPath = os.getenv("PROLOG_PATH") if os.getenv("PROLOG_PATH") else None
        self.prologArgs = prologArgs

    @staticmethod
    def parametrize(
        testcase_klass,
        essentialOnly=False,
        failOnUnlikely=False,
        test_item_name=None,
        launchServer=True,
        useUnixDomainSocket=None,
        serverPort=None,
        password=None,
    ):
        """Create a suite containing all tests taken from the given
        subclass, passing them the parameter 'param'.
        """
        testloader = unittest.TestLoader()
        testnames = testloader.getTestCaseNames(testcase_klass)
        suite = unittest.TestSuite()
        if test_item_name is None:
            for name in testnames:
                suite.addTest(
                    testcase_klass(
                        name,
                        essentialOnly=essentialOnly,
                        failOnUnlikely=failOnUnlikely,
                        launchServer=launchServer,
                        useUnixDomainSocket=useUnixDomainSocket,
                        serverPort=serverPort,
                        password=password,
                    )
                )
        else:
            suite.addTest(
                testcase_klass(
                    test_item_name,
                    essentialOnly=essentialOnly,
                    failOnUnlikely=failOnUnlikely,
                    launchServer=launchServer,
                    useUnixDomainSocket=useUnixDomainSocket,
                    serverPort=serverPort,
                    password=password,
                )
            )

        return suite


class TestPrologMQI(ParametrizedTestCase):
    def setUp(self):
        self.initialProcessCount = self.process_count("swipl")

    def tearDown(self):
        # Make sure we aren't leaving processes around
        # Give the process a bit to exit
        # Since this takes time, and won't work when running in SWI Prolog build process
        # Turn it off if essentialOnly
        if not essentialOnly:
            count = 0
            while count < 10:
                currentCount = self.process_count("swipl")
                if currentCount == self.initialProcessCount:
                    break
                else:
                    sleep(1)
                count += 1
            self.assertEqual(currentCount, self.initialProcessCount)

        # If we're using a Unix Domain Socket, make sure the file was cleaned up
        self.assertTrue(
            self.useUnixDomainSocket is None
            or not os.path.exists(self.useUnixDomainSocket)
        )

    def process_count(self, process_name):
        if os.name == "nt":
            call = "TASKLIST", "/FI", "imagename eq %s" % process_name + ".exe"
            # use buildin check_output right away
            output = subprocess.check_output(call).decode()
            # check each line for process name
            count = 0
            for line in output.strip().split("\r\n"):
                if line.lower().startswith(process_name.lower()):
                    count += 1

            return count
        else:
            try:
                with subprocess.Popen(
                    ["pgrep", process_name], stdout=subprocess.PIPE
                ) as process:
                    data = process.stdout.readlines()
                    return len(data)
            except FileNotFoundError as err:
                # Some systems don't have pgrep, so just return -1 so that the before
                # and after process counts match (but are ignored)
                return -1

    def thread_failure_reason(self, client, threadID, secondsTimeout):
        count = 0
        while True:
            if count >= secondsTimeout:
                self.fail(f"ThreadID: '{threadID}' did not stop.")

            # Thread has exited if thread_property(GoalID, status(PropertyGoal)) and PropertyGoal \== running OR if we get an exception (meaning the thread is gone)
            result = client.query(
                "GoalID = {}, once((\\+ is_thread(GoalID) ; catch(thread_property(GoalID, status(PropertyGoal)), Exception, true), once(((var(Exception), PropertyGoal \\== running) ; nonvar(Exception)))))".format(
                    threadID
                )
            )
            if result is False:
                count += 1
                sleep(1)
            else:
                # If the thread was aborted keep trying since it will spuriously appear and then disappear
                reason = result[0]["PropertyGoal"]
                # Should be this but - Workaround SWI Prolog bug: https://github.com/SWI-Prolog/swipl-devel/issues/852
                # Joining crashes Prolog in the way the code joins and so we will have extra threads that have exited reported by thread_property
                # just treat them as gone
                # if prolog_name(reason) == "exception" and prolog_args(reason)[0] == "$aborted":
                #     continue
                # else:
                #     return reason
                if (
                    prolog_name(reason) == "exception"
                    and prolog_args(reason)[0] == "$aborted"
                ):
                    return "_"
                else:
                    return reason

    # Wait for the threads to exit and return the reason for exit
    # will be "_" if they exited in an expected way
    def thread_failure_reasons(self, client, threadIDList, secondsTimeout):
        reasons = []
        for threadID in threadIDList:
            reason = self.thread_failure_reason(client, threadID, secondsTimeout)
            reasons.append(reason)

        return reasons

    def assertThreadExitExpected(self, client, threadIDList, timeout):
        reasonList = self.thread_failure_reasons(client, threadIDList, timeout)
        for reason in reasonList:
            # They should exit in an expected way.
            # However, it can take a while for a thread to exit, especially on a heavily loaded system
            # which means the test might (wrongly) fail.
            # So, use an environment variable to control whether the test fails or just prints a warning
            if reason != "_":
                if self.failOnUnlikely:
                    self.assertEqual(reason, "_")
                else:
                    print(f"WARNING: Threads '{threadIDList}' did not exit in the allotted time. This can happen if the system is heavily loaded and is thus a warning by default. To turn this into a failure set the environment variable 'SWIPL_TEST_FAIL_ON_UNLIKELY=y'.")

    def thread_list(self, prologThread):
        result = prologThread.query("thread_property(ThreadID, status(Status))")
        testThreads = []
        for item in result:
            # Should be this but - Workaround SWI Prolog bug: https://github.com/SWI-Prolog/swipl-devel/issues/852
            # Joining crashes Prolog in the way the code joins and so we will have extra threads that have exited reported by thread_property
            # just treat them as gone
            # testThreads.append(item["ThreadID"] + ":" + str(item["Status"]))
            if prolog_name(item["Status"]) == "true" or (
                prolog_name(item["Status"]) == "exception"
                and prolog_args(item["Status"])[0] == "$aborted"
            ):
                continue
            else:
                testThreads.append(item["ThreadID"] + ":" + str(item["Status"]))

        return testThreads

    def wait_for_new_threads_exit(self, client, beforeThreadList, afterThreadList, timeout):
        beforeThreadSet = set(beforeThreadList)
        afterThreadSet = set(afterThreadList)
        difference = beforeThreadSet.difference(afterThreadSet)
        runningThreads = []
        for threadStatus in difference:
            statusParts = threadStatus.split(":")
            assert len(statusParts) == 2
            if statusParts[1] == "running":
                runningThreads.append(statusParts[0])

        # Wait for all the new threads to exit
        self.assertThreadExitExpected(client, runningThreads, timeout)

    def round_trip_prolog(self, client, testTerm, expectedText=None):
        if expectedText is None:
            expectedText = testTerm
        result = client.query(f"X = {testTerm}")
        term = result[0]["X"]
        convertedTerm = json_to_prolog(term)
        assert convertedTerm == expectedText

    def sync_query_timeout(self, prologThread, sleepForSeconds, queryTimeout):
        # Query that times out")
        caughtException = False
        try:
            result = prologThread.query(
                f"sleep({sleepForSeconds})", query_timeout_seconds=queryTimeout
            )
        except PrologQueryTimeoutError as error:
            caughtException = True
        assert caughtException

    def async_query_timeout(self, prologThread, sleepForSeconds, queryTimeout):
        # async query with all results that times out on second of three results")
        prologThread.query_async(
            f"(member(X, [Y=a, sleep({sleepForSeconds}), Y=b]), X)",
            query_timeout_seconds=queryTimeout,
        )
        try:
            result = prologThread.query_async_result()
        except PrologQueryTimeoutError as error:
            caughtException = True
        assert caughtException

        # Calling cancel after the goal times out after one successful iteration")
        prologThread.query_async(
            f"(member(X, [Y=a, sleep({sleepForSeconds}), Y=b]), X)",
            query_timeout_seconds=queryTimeout,
            find_all=False,
        )
        sleep(sleepForSeconds + 1)
        prologThread.cancel_query_async()
        results = []
        while True:
            try:
                result = prologThread.query_async_result()
            except PrologQueryTimeoutError as error:
                results.append("time_limit_exceeded")
                break
            if result is None:
                break
            results.append(result)
        self.assertEqual(
            [
                [{"X": {"args": ["a", "a"], "functor": "="}, "Y": "a"}],
                "time_limit_exceeded",
            ],
            results,
        )

    def test_json_to_prolog(self):
        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:
            with server.create_thread() as client:
                # Test non-quoted terms
                self.round_trip_prolog(client, "a")
                self.round_trip_prolog(client, "1")
                self.round_trip_prolog(client, "1.1")
                self.round_trip_prolog(client, "a(b)")
                self.round_trip_prolog(client, "a(b, c)")
                self.round_trip_prolog(client, "[a(b)]")
                self.round_trip_prolog(client, "[a(b), b(c)]")
                self.round_trip_prolog(client, "[a(b(d)), b(c)]")
                self.round_trip_prolog(client, "[2, 1.1]")

                # Test variables
                self.round_trip_prolog(client, "[_1, _a, Auto]", "[A, B, C]")
                self.round_trip_prolog(client, "_")
                self.round_trip_prolog(client, "_1", "A")
                self.round_trip_prolog(client, "_1a", "A")

                # Test quoting terms
                # Terms that do not need to be quoted round trip without quoting")
                self.round_trip_prolog(client, "a('b')", "a(b)")
                self.round_trip_prolog(client, "a('_')", "a(_)")
                # These terms all need quoting
                self.round_trip_prolog(client, "a('b A')")
                self.round_trip_prolog(client, "a('1b')")
                self.round_trip_prolog(client, "'a b'(['1b', 'a b'])")

    def test_sync_query(self):
        if self.essentialOnly:
            print("skipped", flush=True, end=" ")
            return

        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:

            with server.create_thread() as client:
                # Most basic query with single answer and no free variables
                result = client.query("atom(a)")
                assert True is result

                # Most basic query with multiple answers and no free variables
                client.query(
                    "(retractall(noFreeVariablesMultipleResults), assert((noFreeVariablesMultipleResults :- member(_, [1, 2, 3]))))"
                )
                result = client.query("noFreeVariablesMultipleResults")
                assert [True, True, True] == result

                # Use characters that are encoded in UTF8 in: a one byte (1) two bytes (©) and three bytes (≠)
                # To test message format and make sure it handles non-ascii characters
                client.query(
                    "retractall(oneFreeVariableMultipleResults), assert((oneFreeVariableMultipleResults(X) :- member(X, [1, '©', '≠'])))"
                )
                result = client.query("oneFreeVariableMultipleResults(X)")
                assert [{'X': 1}, {'X': '©'}, {'X': '≠'}] == result

                # Most basic query with single answer and two free variables
                client.query(
                    "(retractall(twoFreeVariablesOneResult(X, Y)), assert((twoFreeVariablesOneResult(X, Y) :- X = 1, Y = 1)))"
                )
                result = client.query("twoFreeVariablesOneResult(X, Y)")
                assert [{"X": 1, "Y": 1}] == result

                # Most basic query with multiple answers and two free variables
                client.query(
                    "(retractall(twoFreeVariablesMultipleResults(X, Y)), assert((twoFreeVariablesMultipleResults(X, Y) :- member(X-Y, [1-1, 2-2, 3-3]))))"
                )
                result = client.query("twoFreeVariablesMultipleResults(X, Y)")
                assert [{"X": 1, "Y": 1}, {"X": 2, "Y": 2}, {"X": 3, "Y": 3}] == result

                # Query that that has a parse error
                caughtException = False
                try:
                    result = client.query("member(X, [first, second, third]")
                except PrologError as error:
                    assert error.is_prolog_exception("syntax_error")
                    caughtException = True
                assert caughtException

                self.sync_query_timeout(client, sleepForSeconds=3, queryTimeout=1)

                # Query that throws
                caughtException = False
                try:
                    result = client.query("throw(test)")
                except PrologError as error:
                    assert error.is_prolog_exception("test")
                    caughtException = True
                assert caughtException

    def test_sync_query_slow(self):
        if self.essentialOnly:
            print("skipped", flush=True, end=" ")
            return

        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:
            with server.create_thread() as client:
                # query that is long enough to send heartbeats but eventually succeeds
                self.assertTrue(client.query("sleep(5)"))
                self.assertGreater(client._heartbeat_count, 0)

    def test_async_query(self):
        if self.essentialOnly:
            print("skipped", flush=True, end=" ")
            return
        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:
            with server.create_thread() as client:
                # Cancelling while nothing is happening should throw
                caughtException = False
                try:
                    client.cancel_query_async()
                except PrologNoQueryError as error:
                    assert error.is_prolog_exception("no_query")
                    caughtException = True
                assert caughtException

                # Getting a result when no query running should throw
                caughtException = False
                try:
                    client.query_async_result()
                except PrologNoQueryError as error:
                    assert error.is_prolog_exception("no_query")
                    caughtException = True
                assert caughtException

                ##########
                # Async queries with all results
                ##########

                # Most basic async query with all results and no free variables
                client.query_async("atom(a)", find_all=True)
                result = client.query_async_result()
                assert True == result

                # Use characters that are encoded in UTF8 in: a one byte (1) two bytes (©) and three bytes (≠)
                # To test message format and make sure it handles non-ascii characters
                client.query_async("member(X, [1, ©, ≠])")
                result = client.query_async_result()
                assert [{"X": 1}, {"X": "©"}, {"X": "≠"}] == result

                # async query with all results that gets cancelled while goal is executing
                client.query_async("(member(X, [Y=a, sleep(3), Y=b]), X)")
                client.cancel_query_async()
                try:
                    result = client.query_async_result()
                except PrologQueryCancelledError as error:
                    assert error.is_prolog_exception("cancel_goal")
                    caughtException = True
                assert caughtException

                # async query with all results that throws
                client.query_async("throw(test)")
                try:
                    result = client.query_async_result()
                except PrologError as error:
                    assert error.is_prolog_exception("test")
                    caughtException = True
                assert caughtException

                ##########
                # Async queries with individual results
                ##########

                # async query that has a parse error
                query = "member(X, [first, second, third]"
                caughtException = False
                try:
                    client.query_async(query)
                except PrologError as error:
                    assert error.is_prolog_exception("syntax_error")
                    caughtException = True
                assert caughtException

                # Use characters that are encoded in UTF8 in: a one byte (1) two bytes (©) and three bytes (≠)
                # To test message format and make sure it handles non-ascii characters
                client.query_async("member(X, [1, ©, ≠])", find_all=False)
                results = []
                while True:
                    result = client.query_async_result()
                    if result is None:
                        break
                    results.append(result[0])
                assert [{"X": 1}, {"X": "©"}, {"X": "≠"}] == results

                # Async query with individual results that times out on second of three results
                client.query_async(
                    "(member(X, [Y=a, sleep(3), Y=b]), X)",
                    query_timeout_seconds=1,
                    find_all=False,
                )
                results = []
                while True:
                    try:
                        result = client.query_async_result()
                    except PrologError as error:
                        results.append(error.prolog())
                        break
                    if result is None:
                        break
                    results.append(result[0])
                assert [
                    {"X": {"args": ["a", "a"], "functor": "="}, "Y": "a"},
                    "time_limit_exceeded",
                ] == results

                # Async query that is cancelled after retrieving first result but while the query is running
                client.query_async(
                    "(member(X, [Y=a, sleep(3), Y=b]), X)", find_all=False
                )
                result = client.query_async_result()
                assert [{"X": {"args": ["a", "a"], "functor": "="}, "Y": "a"}] == result
                client.cancel_query_async()
                try:
                    result = client.query_async_result()
                except PrologQueryCancelledError as error:
                    assert error.is_prolog_exception("cancel_goal")
                    caughtException = True
                assert caughtException

                # Calling cancel after the goal is finished and results have been retrieved
                client.query_async("(member(X, [Y=a, Y=b, Y=c]), X)", find_all=True)
                sleep(1)
                result = client.query_async_result()
                assert [
                    {"X": {"args": ["a", "a"], "functor": "="}, "Y": "a"},
                    {"X": {"args": ["b", "b"], "functor": "="}, "Y": "b"},
                    {"X": {"args": ["c", "c"], "functor": "="}, "Y": "c"},
                ] == result
                caughtException = False
                try:
                    client.cancel_query_async()
                except PrologNoQueryError as error:
                    assert error.is_prolog_exception("no_query")
                    caughtException = True
                assert caughtException

                # async query with separate results that throws
                client.query_async("throw(test)", find_all=False)
                try:
                    result = client.query_async_result()
                except PrologError as error:
                    assert error.is_prolog_exception("test")
                    caughtException = True
                assert caughtException

    def test_async_query_slow(self):
        if self.essentialOnly:
            print("skipped", flush=True, end=" ")
            return

        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:
            with server.create_thread() as client:
                # Async query that checks for second result before it is available
                client.query_async(
                    "(member(X, [Y=a, sleep(3), Y=b]), X)",
                    query_timeout_seconds=10,
                    find_all=False,
                )
                results = []
                resultNotAvailable = False
                while True:
                    try:
                        result = client.query_async_result(0)
                        if result is None:
                            break
                        else:
                            results.append(result[0])
                    except PrologResultNotAvailableError as error:
                        resultNotAvailable = True

                assert (
                    resultNotAvailable
                    and [
                        {"X": {"args": ["a", "a"], "functor": "="}, "Y": "a"},
                        {"X": {"args": [3], "functor": "sleep"}, "Y": "_"},
                        {"X": {"args": ["b", "b"], "functor": "="}, "Y": "b"},
                    ]
                    == results
                )

                self.async_query_timeout(client, 3, 1)

    def test_protocol_edge_cases(self):
        if self.essentialOnly:
            print("skipped", flush=True, end=" ")
            return

        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:
            with server.create_thread() as client:
                # Call two async queries in a row. Should work and return the second results at least 1 heartbeat should be sent
                # in the response
                client.query_async(
                    "(member(X, [Y=a, Y=b, Y=c]), X), sleep(3)", find_all=False
                )
                client.query_async("(member(X, [Y=d, Y=e, Y=f]), X)", find_all=False)
                self.assertGreater(client._heartbeat_count, 0)
                results = []
                while True:
                    result = client.query_async_result()
                    if result is None:
                        break
                    results.append(result[0])
                assert [
                    {"X": {"args": ["d", "d"], "functor": "="}, "Y": "d"},
                    {"X": {"args": ["e", "e"], "functor": "="}, "Y": "e"},
                    {"X": {"args": ["f", "f"], "functor": "="}, "Y": "f"},
                ] == results

                # Call sync while async is pending, should work and return sync call results
                client.query_async("(member(X, [Y=a, Y=b, Y=c]), X)", find_all=False)
                results = client.query("(member(X, [Y=d, Y=e, Y=f]), X)")
                assert [
                    {"X": {"args": ["d", "d"], "functor": "="}, "Y": "d"},
                    {"X": {"args": ["e", "e"], "functor": "="}, "Y": "e"},
                    {"X": {"args": ["f", "f"], "functor": "="}, "Y": "f"},
                ] == results

    def test_connection_close_with_running_query(self):
        if self.essentialOnly:
            print("skipped", flush=True, end=" ")
            return

        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:
            with server.create_thread() as monitorThread:
                # Closing a connection with an synchronous query running should abort the query and terminate the threads expectedly
                with server.create_thread() as prologThread:
                    # Run query in a thread since it is synchronous and we want to cancel before finished
                    def TestThread(prologThread):
                        with suppress(Exception):
                            prologThread.query(
                                "(sleep(10), assert(closeConnectionTestFinished)"
                            )

                    thread = threading.Thread(target=TestThread, args=(prologThread,))
                    thread.start()
                    # Give it time to start
                    sleep(1)
                    # Close the connection while running
                    prologThread.stop()
                    thread.join()
                    self.assertThreadExitExpected(
                        monitorThread,
                        [
                            prologThread.goal_thread_id,
                            prologThread.communication_thread_id,
                        ],
                        5,
                    )
                    # Make sure it didn't finish
                    exceptionCaught = False
                    try:
                        monitorThread.query("closeConnectionTestFinished")
                    except PrologError as error:
                        exceptionCaught = True
                        assert error.is_prolog_exception("existence_error")

                # Closing a connection with an asynchronous query running should abort the query and terminate the threads expectedly
                with server.create_thread() as prologThread:
                    prologThread.query_async(
                        "(sleep(10), assert(closeConnectionTestFinished))"
                    )
                    # Give it time to start the goal
                    sleep(1)

                # left "with" clause so connection is closed, query should be cancelled
                self.assertThreadExitExpected(
                    monitorThread,
                    [prologThread.goal_thread_id, prologThread.communication_thread_id],
                    5,
                )
                # Make sure it didn't finish
                exceptionCaught = False
                try:
                    monitorThread.query("closeConnectionTestFinished")
                except PrologError as error:
                    exceptionCaught = True
                    assert error.is_prolog_exception("existence_error")

    # To prove that threads are running concurrently have them all assert something then wait
    # Then release the mutex
    # then check to see if they all finished
    def test_multiple_connections(self):
        if self.essentialOnly:
            print("skipped", flush=True, end=" ")
            return

        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:
            with server.create_thread() as monitorThread:
                with server.create_thread() as controlThread:
                    # Will keep the mutex since the thread is kept alive
                    try:
                        controlThread.query(
                            "mutex_create(test), mutex_lock(test), assert(started(-1)), assert(ended(-1))"
                        )
                        prologThreads = []
                        for index in range(0, 5):
                            prologThread = server.create_thread()
                            prologThread.start()
                            prologThread.query_async(
                                f"assert({'started(' + str(index) + ')'}), with_mutex(test, assert({'ended(' + str(index) + ')'}))"
                            )
                            prologThreads.append(prologThread)

                        # Give time to get to mutex
                        sleep(3)

                        # now make sure they all started but didn't end since the mutex hasn't been released
                        startResult = monitorThread.query(
                            "findall(Value, started(Value), StartedList), findall(Value, ended(Value), EndedList)"
                        )
                        startedList = startResult[0]["StartedList"]
                        endedList = startResult[0]["EndedList"]
                        self.assertEqual(startedList.sort(), [-1, 0, 1, 2, 3, 4].sort())
                        self.assertEqual(endedList, [-1])

                        # release the mutex and delete the data
                        controlThread.query("mutex_unlock(test)")

                        # They should have ended now
                        startResult = monitorThread.query(
                            "findall(Value, ended(Value), EndedList)"
                        )
                        endedList = startResult[0]["EndedList"]
                        self.assertEqual(endedList.sort(), [-1, 0, 1, 2, 3, 4].sort())
                    finally:
                        # and destroy it
                        controlThread.query(
                            "mutex_destroy(test), retractall(ended(_)), retractall(started(_))"
                        )

    def test_multiple_serial_connections(self):
        if self.essentialOnly:
            print("skipped", flush=True, end=" ")
            return

        # Multiple connections can run serially
        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:
            with server.create_thread() as prologThread:
                result = prologThread.query("true")
                self.assertEqual(result, True)
            sleep(1)
            with server.create_thread() as prologThread:
                result = prologThread.query("true")
                self.assertEqual(result, True)
            sleep(1)
            with server.create_thread() as prologThread:
                result = prologThread.query("true")
                self.assertEqual(result, True)

    def test_goal_thread_failure(self):
        if self.essentialOnly:
            print("skipped", flush=True, end=" ")
            return

        # If the goal thread fails, we should get a specific exception and the thread should be left for inspection
        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:
            with server.create_thread() as prologThread:
                # Force the goal thread to throw outside of the "safe zone" and shutdown unexpectedly
                prologThread._send("testThrowGoalThread(test_exception).\n")
                result = prologThread._receive()
                # give it time to process
                sleep(2)

                # The next query should get a final exception
                exceptionHandled = False
                try:
                    result = prologThread.query("true")
                except PrologConnectionFailedError as error:
                    assert error.is_prolog_exception("connection_failed")
                    exceptionHandled = True
                assert exceptionHandled

            # At this point the server communication thread has failed and stopped the server since we launched with
            # haltOnCommunicationFailure(true), so this should fail.  Finding a reliable way to detect if the process is gone
            # that works cross platform was very hard.  The alternative which is to try to connect and fail after a timeout
            # was surprisingly also hard.  So, for now, we're not verifying that.

    def test_quit(self):
        # Sending quit should shutdown the server")
        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:
            with server.create_thread() as prologThread:
                prologThread.halt_server()
                # Finding a reliable way to detect if the process is gone
                # that works cross platform was very hard.  The alternative which is to try to connect and fail after a timeout
                # was surprisingly also hard.  So, for now, we're not verifying that.

    def test_unknown_command(self):
        # Sending an unknown command should throw")
        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:
            with server.create_thread() as prologThread:
                # Force the goal thread to throw outside of the "safe zone" and shutdown unexpectedly
                prologThread._send("foo.\n")
                result = json.loads(prologThread._receive())
                assert (
                    prolog_name(result) == "exception"
                    and prolog_name(prolog_args(result)[0]) == "unknownCommand"
                )

    def test_server_options_and_shutdown(self):
        global secondsTimeoutForThreadExit
        try:
            with PrologMQI(
                self.launchServer,
                self.serverPort,
                self.password,
                self.useUnixDomainSocket,
                prolog_path=self.prologPath,
            ) as server:
                with server.create_thread() as monitorThread:
                    # Record the threads that are running, but give a pause so any threads created by the server on startup
                    # can get closed down
                    initialThreads = self.thread_list(monitorThread)
                    socketPort = 4250

                    # password() should be used if supplied.
                    result = monitorThread.query(
                        "mqi_start([port(Port), password(testpassword), server_thread(ServerThreadID)])"
                    )
                    serverThreadID = result[0]["ServerThreadID"]
                    port = result[0]["Port"]
                    with PrologMQI(
                        launch_mqi=False,
                        port=port,
                        password="testpassword",
                        prolog_path=self.prologPath,
                    ) as newServer:
                        with newServer.create_thread() as prologThread:
                            result = prologThread.query("true")
                            self.assertEqual(result, True)
                    result = monitorThread.query(f"mqi_stop({serverThreadID})")
                    self.assertEqual(result, True)
                    afterShutdownThreads = self.thread_list(monitorThread)
                    self.wait_for_new_threads_exit(monitorThread, initialThreads, afterShutdownThreads, secondsTimeoutForThreadExit)

                    if os.name != "nt":
                        # unixDomainSocket() should be used if supplied (non-windows).
                        socketPath = mkdtemp()
                        unixDomainSocket = PrologMQI.unix_domain_socket_file(socketPath)
                        result = monitorThread.query(
                            f"mqi_start([unix_domain_socket('{unixDomainSocket}'), password(testpassword), server_thread(ServerThreadID)])"
                        )
                        serverThreadID = result[0]["ServerThreadID"]
                        with PrologMQI(
                            launch_mqi=False,
                            unix_domain_socket=unixDomainSocket,
                            password="testpassword",
                            prolog_path=self.prologPath,
                        ) as newServer:
                            with newServer.create_thread() as prologThread:
                                result = prologThread.query("true")
                                self.assertEqual(result, True)
                        result = monitorThread.query(
                            f"mqi_stop({serverThreadID})"
                        )
                        self.assertEqual(result, True)
                        afterShutdownThreads = self.thread_list(monitorThread)
                        self.wait_for_new_threads_exit(monitorThread, initialThreads, afterShutdownThreads, secondsTimeoutForThreadExit)
                        assert not os.path.exists(unixDomainSocket)

                        # unixDomainSocket() should be generated if asked for (non-windows).
                        result = monitorThread.query(
                            "mqi_start([unix_domain_socket(Socket), password(testpassword), server_thread(ServerThreadID)])"
                        )
                        serverThreadID = result[0]["ServerThreadID"]
                        unixDomainSocket = result[0]["Socket"]
                        with PrologMQI(
                            launch_mqi=False,
                            unix_domain_socket=unixDomainSocket,
                            password="testpassword",
                            prolog_path=self.prologPath,
                        ) as newServer:
                            with newServer.create_thread() as prologThread:
                                result = prologThread.query("true")
                                self.assertEqual(result, True)
                        result = monitorThread.query(
                            f"mqi_stop({serverThreadID})"
                        )
                        self.assertEqual(result, True)
                        afterShutdownThreads = self.thread_list(monitorThread)
                        self.wait_for_new_threads_exit(monitorThread, initialThreads, afterShutdownThreads, secondsTimeoutForThreadExit)
                        # Temp Socket should not exist
                        assert not os.path.exists(unixDomainSocket)
                        # Neither should Temp directory
                        assert not os.path.exists(Path(unixDomainSocket).parent)

                    # runServerOnThread(false) should block until the server is shutdown.
                    # Create a new connection that we block starting a new server
                    with server.create_thread() as blockedThread:
                        blockedThread.query_async(
                            "mqi_start([port({}), password(testpassword), run_server_on_thread(false), server_thread(testServerThread)])".format(
                                socketPort
                            )
                        )
                        # Wait for the server to start
                        sleep(1)

                        # Make sure we are still blocked
                        exceptionCaught = False
                        try:
                            blockedThread.query_async_result(wait_timeout_seconds=0)
                        except PrologResultNotAvailableError:
                            exceptionCaught = True
                        assert exceptionCaught

                        # Ensure the server started by sending it a query
                        with PrologMQI(
                            launch_mqi=False,
                            port=socketPort,
                            password="testpassword",
                            prolog_path=self.prologPath,
                        ) as newServer:
                            with newServer.create_thread() as prologThread:
                                result = prologThread.query("true")
                                self.assertEqual(result, True)
                        # Make sure we are still blocked
                        exceptionCaught = False
                        try:
                            blockedThread.query_async_result(wait_timeout_seconds=0)
                        except PrologResultNotAvailableError:
                            exceptionCaught = True
                        assert exceptionCaught

                        # Now shut it down by cancelling the query and running stop
                        blockedThread.cancel_query_async()
                        result = monitorThread.query(
                            f"mqi_stop({blockedThread.communication_thread_id})"
                        )
                        self.assertEqual(result, True)

                    # And make sure all the threads went away
                    afterShutdownThreads = self.thread_list(monitorThread)
                    self.wait_for_new_threads_exit(monitorThread, initialThreads, afterShutdownThreads, secondsTimeoutForThreadExit)

                    # Launching this library itself and stopping in the debugger tests writeConnectionValues() and ignoreSigint and haltOnConnectionFailure internal features automatically

        except Exception as e:
            if self.failOnUnlikely:
                # Rethrow the exception if we are configured to fail on unlikely tests failures
                raise
            else:
                stackTrace = ''.join(traceback.format_exception(etype=type(e), value=e, tb=e.__traceback__))
                print(
                    f"WARNING: {e} at {stackTrace}.\n This can happen if the system is heavily loaded and is thus a warning by default. To turn this into a failure set the environment variable 'SWIPL_TEST_FAIL_ON_UNLIKELY=y'."
                )

    def test_server_options_and_shutdown_slow(self):
        global secondsTimeoutForThreadExit
        if self.essentialOnly:
            print("skipped", flush=True, end=" ")
            return

        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:
            with server.create_thread() as monitorThread:
                # Record the threads that are running, but give a pause so any threads created by the server on startup
                # can get closed down
                initialThreads = self.thread_list(monitorThread)

                # When starting a server, some variables can be filled in with defaults. Also: only the server thread should be created
                # Launch the new server with appropriate options specified with variables to make sure they get filled in
                if os.name == "nt":
                    result = monitorThread.query(
                        "mqi_start([port(Port), server_thread(ServerThreadID), password(Password)])"
                    )
                    optionsDict = result[0]
                    assert (
                        "Port" in optionsDict
                        and "ServerThreadID" in optionsDict
                        and "Password" in optionsDict
                    )
                else:
                    result = monitorThread.query(
                        "mqi_start([port(Port), server_thread(ServerThreadID), password(Password), unix_domain_socket(Unix)])"
                    )
                    optionsDict = result[0]
                    assert (
                        "Port" in optionsDict
                        and "ServerThreadID" in optionsDict
                        and "Password" in optionsDict
                        and "Unix" in optionsDict
                    )

                # Get the new threadlist
                result = monitorThread.query(
                    "thread_property(ThreadID, status(Status))"
                )
                testThreads = self.thread_list(monitorThread)

                # Only a server thread should have been started
                assert len(testThreads) - len(initialThreads) == 1

                # stop_language_server should remove all (and only) created threads and the Unix Domain File (which is tested on self.tearDown())
                result = monitorThread.query(
                    f"mqi_stop({optionsDict['ServerThreadID']})"
                )
                sleep(2)
                afterShutdownThreads = self.thread_list(monitorThread)
                self.wait_for_new_threads_exit(monitorThread, initialThreads, afterShutdownThreads, secondsTimeoutForThreadExit)

                # queryTimeout() supplied at startup should apply to queries by default. password() and port() should be used if supplied.
                socketPort = 4250
                result = monitorThread.query(
                    "mqi_start([query_timeout(1), port({}), password(testpassword), server_thread(ServerThreadID)])".format(
                        socketPort
                    )
                )
                serverThreadID = result[0]["ServerThreadID"]
                with PrologMQI(
                    launch_mqi=False,
                    port=socketPort,
                    password="testpassword",
                    prolog_path=self.prologPath,
                ) as newServer:
                    with newServer.create_thread() as prologThread:
                        self.sync_query_timeout(
                            prologThread, sleepForSeconds=2, queryTimeout=None
                        )
                        self.async_query_timeout(
                            prologThread, sleepForSeconds=2, queryTimeout=None
                        )
                result = monitorThread.query(f"mqi_stop({serverThreadID})")
                self.assertEqual(result, True)
                afterShutdownThreads = self.thread_list(monitorThread)
                self.wait_for_new_threads_exit(monitorThread, initialThreads, afterShutdownThreads, secondsTimeoutForThreadExit)

                # Shutting down a server with an active query should abort it and close all threads properly.
                result = monitorThread.query(
                    "mqi_start([port({}), password(testpassword), server_thread(ServerThreadID)])".format(
                        socketPort
                    )
                )
                serverThreadID = result[0]["ServerThreadID"]
                with PrologMQI(
                    launch_mqi=False,
                    port=socketPort,
                    password="testpassword",
                    prolog_path=self.prologPath,
                ) as newServer:
                    with newServer.create_thread() as prologThread:
                        prologThread.query_async("sleep(20)")
                # Wait for query to start running
                sleep(2)
                result = monitorThread.query(f"mqi_stop({serverThreadID})")
                assert result is True
                afterShutdownThreads = self.thread_list(monitorThread)
                self.wait_for_new_threads_exit(monitorThread, initialThreads, afterShutdownThreads, secondsTimeoutForThreadExit)

    def test_unix_domain_socket_embedded(self):
        if os.name != "nt":
            with PrologMQI(
                launch_mqi=True,
                unix_domain_socket="",
                password="testpassword",
                prolog_path=self.prologPath,
            ) as newServer:
                with newServer.create_thread() as prologThread:
                    result = prologThread.query("true")
                    self.assertEqual(result, True)


    def test_python_classes(self):
        # Using a thread without starting it should start the server
        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:
            prolog_thread = PrologThread(server)
            self.assertTrue(prolog_thread.query("true"))
            pid = server.process_id()

        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:
            prolog_thread = PrologThread(server)
            self.assertIsNone(prolog_thread.query_async("true"))
            pid = server.process_id()

        # Start a thread twice is ignored
        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:
            with PrologThread(server) as prolog_thread:
                prolog_thread.start()
                self.assertTrue(prolog_thread.query("true"))

        # Setting a Unix Domain Socket on windows should raise
        if os.name == "nt":
            exceptionCaught = False
            try:
                with PrologMQI(
                    unix_domain_socket="C:\temp.socket", prolog_path=self.prologPath
                ) as server:
                    pass
            except ValueError:
                exceptionCaught = True
            self.assertTrue(exceptionCaught)

        # Setting port and unix_domain_socket should raise
        exceptionCaught = False
        try:
            with PrologMQI(
                port=4242, unix_domain_socket="", prolog_path=self.prologPath
            ):
                pass
        except ValueError:
            exceptionCaught = True
        self.assertTrue(exceptionCaught)

        # Setting output_file when launch_mqi is False should raise
        exceptionCaught = False
        try:
            with PrologMQI(
                output_file_name="/test.txt",
                launch_mqi=False,
                prolog_path=self.prologPath,
            ):
                pass
        except ValueError:
            exceptionCaught = True
        self.assertTrue(exceptionCaught)

    def test_debugging_options(self):
        if self.essentialOnly:
            print("skipped", flush=True, end=" ")
            return

        tempDir = gettempdir()
        # Put a space in to make sure escaping is working
        tempFile = os.path.join(tempDir, "swiplserver output.txt")
        try:
            os.remove(tempFile)
        except:
            pass
        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            mqi_traces="_",
            output_file_name=tempFile,
            prolog_path=self.prologPath,
        ) as server:
            with PrologThread(server) as prolog_thread:
                prolog_thread.query("true")

        # Just make sure we have some output in the file
        with open(tempFile) as f:
            lines = f.readlines()
            self.assertTrue(len(lines) > 10)

        os.remove(tempFile)

    def test_write_output_to_file_in_embedded_mode(self):
        # Ensure that using embedded mode still sends password and port to STDOUT before redirecting output or
        # embedded mode will fail
        tempDir = gettempdir()
        tempFile = os.path.join(tempDir, "swiplserveroutput.txt")

        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            output_file_name=tempFile,
            mqi_traces="_",
            prolog_path=self.prologPath,
        ) as server:
            with PrologThread(server) as prolog_thread:
                self.assertTrue(prolog_thread.query("true"))

    def test_connection_failure(self):
        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:
            with PrologThread(server) as prolog_thread:
                # Closing the socket without sending "close.\n" should shutdown and exit the process
                prolog_thread._socket.close()
                # Set this or we will get exceptions on close
                server.connection_failed = True

    def skip_test_protocol_overhead(self):
        with PrologMQI(
            self.launchServer,
            self.serverPort,
            self.password,
            self.useUnixDomainSocket,
            prolog_path=self.prologPath,
        ) as server:
            with PrologThread(server) as prolog_thread:
                iterations = 10000
                bestResult = None
                gc.disable()  # so it doesn't collect during the run
                # Numbers vary widely due to many things including GC so run many times and report the best number
                for runIndex in range(0, 10):
                    startEvalTime = perf_counter()
                    for count in range(0, iterations):
                        prolog_thread.query("true")
                    thisResult = perf_counter() - startEvalTime
                    print(f"Measured value {thisResult}")
                    if bestResult is None or thisResult < bestResult:
                        bestResult = thisResult

                gc.enable()
                print(
                    f"Best Time to run {iterations} iterations of the Prolog query `true`: {bestResult}"
                )

    # Run a simple query 1000 times to test for leaks
    def skip_test_launch_stress(self):
        for index in range(0, 10000):
            print(index)
            with PrologMQI(
                self.launchServer,
                self.serverPort,
                self.password,
                self.useUnixDomainSocket,
                prolog_path=self.prologPath,
            ) as server:
                with PrologThread(server) as prolog_thread:
                    prolog_thread.query("true")


def run_tcpip_performance_tests(suite):
    suite.addTest(TestPrologMQI("skip_test_protocol_overhead"))


def run_unix_domain_sockets_performance_tests(suite):
    socketPath = os.path.dirname(os.path.realpath(__file__))
    suite.addTest(
        ParametrizedTestCase.parametrize(
            TestPrologMQI,
            test_item_name="skip_test_protocol_overhead",
            launchServer=True,
            useUnixDomainSocket=PrologMQI.unix_domain_socket_file(socketPath),
            serverPort=None,
            password=None,
        )
    )


def load_tests(loader, standard_tests, pattern):
    global essentialOnly
    global failOnUnlikely
    suite = unittest.TestSuite()

    # Run the perf tests
    # Perf tests should only be run one per run as the numbers vary greatly otherwise
    # run_tcpip_performance_tests(suite)
    # run_unix_domain_sockets_performance_tests(suite)

    # Tests a specific test
    # suite.addTest(TestPrologMQI('test_sync_query'))
    # return suite

    # Tests a specific test with parameters set
    # suite.addTest(ParametrizedTestCase.parametrize(TestPrologMQI, test_item_name="test_sync_query", launchServer=False,
    #                                                serverPort=4242, password="debugnow"))

    # Tests a specific test 100 times
    # for index in range(0, 100):
    #     suite.addTest(ParametrizedTestCase.parametrize(TestPrologMQI, test_item_name="test_multiple_connections", launchServer=True, useUnixDomainSocket=None, serverPort=None, password=None))

    # Run full test suite using Unix Domain Sockets when appropriate as "main" way to connect
    # Tests include both Port and Unix Domain socket tests so both are tested in either mode
    if os.name == "nt":
        suite.addTest(
            ParametrizedTestCase.parametrize(
                TestPrologMQI,
                essentialOnly=essentialOnly,
                failOnUnlikely=failOnUnlikely,
                launchServer=True,
                useUnixDomainSocket=None,
                serverPort=None,
                password=None,
            )
        )
    else:
        socketPath = tempfile.mkdtemp()
        suite.addTest(
            ParametrizedTestCase.parametrize(
                TestPrologMQI,
                essentialOnly=essentialOnly,
                failOnUnlikely=failOnUnlikely,
                launchServer=True,
                useUnixDomainSocket=PrologMQI.unix_domain_socket_file(socketPath),
                serverPort=None,
                password=None,
            )
        )

    return suite


# This code is to allow the runner of the test to set environment variables
# that:
#   - run a smaller set of tests (ESSENTIAL_TESTS_ONLY=True)
#   - set an environment variable that allows tests that can fail in unlikely scenarios to
#       do so (SWIPL_TEST_FAIL_ON_UNLIKELY = y). Without this set, they will simply output a
#       warning to the console
#   - set the path and args to use when PrologServer launches the Prolog process
#       the latter is designed for running in the SWI Prolog build system since
#       it needs certain arguments passed along
failOnUnlikely = os.getenv("SWIPL_TEST_FAIL_ON_UNLIKELY") == "y"
essentialOnly = os.getenv("ESSENTIAL_TESTS_ONLY") == "True"
prologPath = os.getenv("PROLOG_PATH")
prologArgsString = os.getenv("PROLOG_ARGS")
if prologArgsString is not None:
    prologArgs = prologArgsString.split("~|~")
    finalArgs = []
    skip = False
    for index in range(0, len(prologArgs)):
        if skip:
            continue
        if prologArgs[index] in ["-s", "-g"]:
            skip = True
        else:
            finalArgs.append(prologArgs[index])
    prologArgs = finalArgs
else:
    prologArgs = None

# How long to wait for a thread to exit
secondsTimeoutForThreadExit = 60

if __name__ == "__main__":
    print(
        "**** Note that some builds of Prolog will print out messages about\n'Execution Aborted' or 'did not clear exception...' when running tests.  Ignore them."
    )

    # perfLogger = logging.getLogger("swiplserver")
    # perfLogger.setLevel(logging.DEBUG)
    # formatter = logging.Formatter('%(name)s %(asctime)s: %(message)s')
    # file_handler = logging.StreamHandler(sys.stdout)
    # file_handler.setFormatter(formatter)
    # perfLogger.addHandler(file_handler)

    unittest.main(verbosity=2, module="test_prologserver")

    # # unittest.main(verbosity=2, module="test_prologserver", failfast=True)
