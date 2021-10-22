# Machine Query Interface Overview		{#mqi-overview}
The SWI Prolog Machine Query Interface ('MQI') is designed to enable embedding SWI Prolog into just about any programming language (Python, Go, C#, etc) in a straightforward way. It is designed for scenarios that need to use SWI Prolog as a local implementation detail of another language. Think of it as running SWI Prolog "like a library". It can support any programming language that can launch processes, read their STDOUT pipe, and send and receive JSON over TCP/IP. A Python 3 library is included as a part of SWI Prolog, see [Installation Steps for Python](#mqi-python-installation).

Key features of the MQI:

    - Simulates the familiar Prolog "top level" (i.e. the interactive prompt you get when running Prolog: "?-").
    - Always runs queries from a connection on a consistent, single thread for that connection. The application itself can still be multi-threaded by running queries that use the multi-threading Prolog predicates or by opening more than one connection.
    - Runs as a separate dedicated *local* Prolog process to simplify integration (vs. using the C-level SWI Prolog interface). The process is launched and managed by a specific running client (e.g. Python or other language) program.
    - Communicates using sockets and [JSON](https://www.json.org/) encoded as UTF-8 to allow it to work on any platform supported by SWI Prolog. For security reasons, only listens on TCP/IP localhost or Unix Domain Sockets and requires (or generates depending on the options) a password to open a connection.
    - Has a lightweight text-based message format with only 6 commands: run synchronous query, run asynchronous query, retrieve asynchronous results, cancel asynchronous query, close connection and terminate the session.
    - Communicates answers using [JSON](https://www.json.org/), a well-known data format supported by most languages natively or with generally available libraries.


The server can be used in two different modes:

    - *Embedded mode*: This is the main use case for the MQI. The user uses a library (just like any other library in their language of choice). That library integrates the MQI by launching the SWI Prolog process, connecting to it, and wrapping the MQI protocol with a language specific interface.
    - *Standalone mode*: The user still uses a library as above, but launches SWI Prolog independently of the language. The client language library connects to that process. This allows the user to see, interact with, and debug the Prolog process while the library interacts with it.

Note that the MQI is related to the [Pengines library](pengine-references), but where the Pengines library is focused on a client/server, multi-tenet, sandboxed environment, the MQI is local, single tenet and unconstrained. Thus, when the requirement is to embed Prolog within another programming language "like a library", it can be a good solution for exposing the full power of Prolog with low integration overhead.

## Installation Steps for Python {#mqi-python-installation}
A Python 3.x library that integrates Python with SWI Prolog using the Machine Query Interface is included within the `libs` directory of the SWI Prolog installation. It is also available using =|pip install swiplserver|=. See the [Python swiplserver library documentation](https://www.swi-prolog.org/packages/mqi/prologmqi.html) for more information on how to use and install it from either location.

## Installation Steps for Other Languages {#mqi-language-installation}

In general, to use the Machine Query Interface with any programming language:

    1. Install SWI Prolog itself on the machine the application will run on.
    2. Check if your SWI Prolog version includes the MQI by launching it and typing `?- mqi_start([]).` If it can't find it, see below for how to install it.
    3. Ensure that the system path includes a path to the `swipl` executable from that installation.
    4. Make sure the application (really the user that launches the application) has permission to launch the SWI Prolog process. Unless your system is unusually locked down, this should be allowed by default.  If not, you'll need to set the appropriate permissions to allow this.
    5. Install (or write!) the library you'll be using to access the MQI in your language of choice.

If your SWI Prolog version doesn't yet include the MQI:
    1. Download the =|mqi.pl|= file from the [GitHub repository](https://github.com/SWI-Prolog/packages-mqi/blob/master/mqi.pl).
    2. Open an operating system command prompt and go to the directory where you downloaded =|mqi.pl|=.
    3. Run the command below. On Windows the command prompt must be [run as an administrator](https://www.wikihow.com/Run-Command-Prompt-As-an-Administrator-on-Windows). On Mac or Linux, start the command with `sudo` as in ``sudo swipl -s ...``.

~~~
swipl -s mqi.pl -g "mqi:install_to_library('mqi.pl')" -t halt
~~~

## Prolog Language Differences from the Top Level {#mqi-toplevel-differences}

The Machine Query Interface is designed to act like using the ["top level"](quickstart) prompt of SWI Prolog itself (i.e. the "?-" prompt).  If you've built the Prolog part of your application by loading code, running it and debugging it using the normal SWI Prolog top level, integrating it with your native language should be straightforward: simply run the commands you'd normally run on the top level, but now run them using the query APIs provided by the library built for your target language. Those APIs will allow you to send the exact same text to Prolog and they should execute the same way.  Here's an example using the Python `swiplserver` library:

~~~
% Prolog Top Level
?- member(X, [first, second, third]).
X = first ;
X = second ;
X = third.
~~~
~~~
# Python using the swiplserver library
from swiplserver import PrologMQI, PrologThread

with PrologMQI() as mqi:
    with mqi.create_thread() as prolog_thread:
        result = prolog_thread.query("member(X, [first, second, third]).")
        print(result)

first
second
third
~~~

While the query functionality of the MQI does run on a thread, it will always be the *same* thread, and, if you use a single connection, it will only allow queries to be run one at a time, just like the top level. Of course, the queries you send can launch threads, just like the top level, so you are not limited to a single threaded application. There are a few differences from the top level, however:

    - Normally, the SWI Prolog top level runs all user code in the context of a built-in module called "user", as does the MQI. However, the top level allows this to be changed using the module/1 predicate. This predicate has no effect when sent to the MQI.
    - Predefined streams like `user_input` are initially bound to the standard operating system I/O streams (like STDIN) and, since the Prolog process is running invisibly, will obviously not work as expected. Those streams can be changed, however, by issuing commands using system predicates as defined in the SWI Prolog documentation.
    - Every connection to the MQI runs in its own thread, so opening two connections from an application means you are running multithreaded code.

The basic rule to remember is: any predicates designed to interact with or change the default behavior of the top level itself probably won't have any effect.


## Embedded Mode: Integrating the Machine Query Interface Into a New Programming Language {#mqi-embedded-mode}
The most common way to use the Machine Query Interface is to find a library that wraps and exposes it as a native part of another programming language such as the [Python =|swiplserver|= library](#mqi-python-installation). This section describes how to build one if there isn't yet a library for your language.  To do this, you'll need to familiarize yourself with the MQI protocol as described in the `mqi_start/1` documentation. However, to give an idea of the scope of work required, below is a typical interaction done (invisibly to the user) in the implementation of any programming language library:


     1. Launch the SWI Prolog process using (along with any other options the user requests): =|swipl --quiet -g mqi_start -t halt -- --write_connection_values=true|=.  To work, the `swipl` Prolog executable will need to be on the path or the path needs to be specified in the command. This launches SWI Prolog, starts the MQI, and writes the chosen port and password to STDOUT.  This way of launching invokes the mqi_start/0 predicate that turns off the `int` (i.e. Interrupt/SIGINT) signal to Prolog. This is because some languages (such as Python) use that signal during debugging and it would be otherwise passed to the client Prolog process and switch it into the debugger.  See the mqi_start/0 predicate for more information on other command line options.
     2. Read the SWI Prolog STDOUT to retrieve the TCP/IP port and password. They are sent in that order, delimited by '\n'.

~~~
$ swipl --quiet -g mqi_start -t halt -- --write_connection_values=true
54501
185786669688147744015809740744888120144
~~~

    Now the server is started. To create a connection:

     3. Use the language's TCP/IP sockets library to open a socket on the specified port of localhost and send the password as a message. Messages to and from the MQI are in the form =|<stringByteLength>.\n<stringBytes>.\n |= where `stringByteLength` includes the =|.\n|= from the string. For example: =|7.\nhello.\n|= More information on the [message format](#mqi-message-format) is below.
     4. Listen on the socket for a response message of `true([[threads(Comm_Thread_ID, Goal_Thread_ID)]])` (which will be in JSON form) indicating successful creation of the connection.  `Comm_Thread_ID` and `Goal_Thread_ID` are the internal Prolog IDs of the two threads that are used for the connection. They are sent solely for monitoring and debugging purposes.

We can try all of this using the Unix tool `nc` (netcat) (also available for Windows) to interactively connect to the MQI. In `nc` hitting `enter` sends =|\n|= which is what the message format requires. The server responses are show indented inline.

We'll use the port and password that were sent to STDOUT above:
~~~
$ nc 127.0.0.1 54501
41.
185786669688147744015809740744888120144.
    173.
    {
      "args": [
        [
          [
        {
          "args": ["mqi1_conn2_comm", "mqi1_conn2_goal" ],
          "functor":"threads"
        }
          ]
        ]
      ],
      "functor":"true"
    }

~~~

 Now the connection is established. To run queries and shutdown:

     5. Any of the messages described in the [Machine Query Interface messages documentation](#mqi-messages) can now be sent to run queries and retrieve their answers. For example, send the message `run(atom(a), -1)` to run the synchronous query `atom(a)` with no timeout and wait for the response message. It will be `true([[]])` (in JSON form).
     6. Shutting down the connection is accomplished by sending the message `close`, waiting for the response message of `true([[]])` (in JSON form), and then closing the socket using the socket API of the language.  If the socket is closed (or fails) before the `close` message is sent, the default behavior of the MQI is to exit the SWI Prolog process to avoid leaving the process around.  This is to support scenarios where the user is running and halting their language debugger without cleanly exiting.
     7. Shutting down the launched SWI Prolog process is accomplished by sending the `quit` message and waiting for the response message of `true([[]])` (in JSON form). This will cause an orderly shutdown and exit of the process.

Continuing with the `nc` session (the `quit` message isn't shown since the `close` message closes the connection):
~~~
18.
run(atom(a), -1).
    39.
    {"args": [ [ [] ] ], "functor":"true"}
7.
close.
    39.
    {"args": [ [ [] ] ], "functor":"true"}
~~~

Note that Unix Domain Sockets can be used instead of a TCP/IP port. How
to do this is described with mqi_start/1.

Here's the same example running in the R language. Note that this is *not* an example of how to use the MQI from R, it just shows the first code a developer would write as they begin to build a nice library to connect R to Prolog using the MQI:
~~~
# Server run with: swipl mqi.pl --port=40001 --password=123
# R Source
print("# Establish connection")

sck = make.socket('localhost', 40001)

print("# Send password")

write.socket(sck, '5.\n') # message length

write.socket(sck, '123.\n') # password

print(read.socket(sck))

print("# Run query")

query = 'run(member(X, [1, 2, 3]), -1).\n'

write.socket(sck, paste(nchar(query), '.\n', sep='')) # message length

write.socket(sck, query) # query

print(read.socket(sck))

print("# Close session")

close.socket(sck)
~~~
And here's the output:
~~~
[1] "# Establish connection"

[1] "# Send password"

[1] "172.\n{\n "args": [\n [\n [\n\t{\n\t "args": ["mqi1_conn1_comm", "mqi1_conn1_goal" ],\n\t "functor":"threads"\n\t}\n ]\n ]\n ],\n "functor":"true"\n}"

[1] "# Run query"

[1] "188.\n{\n "args": [\n [\n [ {"args": ["X", 1 ], "functor":"="} ],\n [ {"args": ["X", 2 ], "functor":"="} ],\n [ {"args": ["X", 3 ], "functor":"="} ]\n ]\n ],\n "functor":"true"\n}"

[1] "# Close session"
~~~

Other notes about creating a new library to communicate with the MQI:
- Where appropriate, use similar names and approaches to the [Python library](https://github.com/SWI-Prolog/packages-mqi/tree/master/python) when designing your language library. This will give familiarity and faster learning for users that use more than one language.
- Use the `debug/1` predicate described in the `mqi_start/1` documentation to turn on debug tracing. It can really speed up debugging.
- Read the STDOUT and STDERR output of the SWI Prolog process and output them to the debugging console of the native language to help users debug their Prolog application.

## Standalone Mode: Debugging Prolog Code Used in an Application {#mqi-standalone-mode}
When using the Machine Query Interface from another language, debugging the Prolog code itself can often be done by viewing traces from the Prolog native `writeln/1` or `debug/3` predicates. Their output will be shown in the debugger of the native language used.  Sometimes an issue surfaces deep in an application. When this happens, running the application in the native language while setting breakpoints and viewing traces in Prolog itself is often the best debugging approach. Standalone mode is designed for this scenario.

As the MQI is a multithreaded application, debugging the running code requires using the multithreaded debugging features of SWI Prolog as described in the section on ["Debugging Threads"](threaddebug) in the SWI Prolog documentation. A typical flow for Standalone Mode is:

    1. Launch SWI Prolog and call the `mqi_start/1` predicate specifying a port and password. Use the `tdebug/0` predicate to set all threads to debugging mode like this: `tdebug, mqi_start([port(4242), password(debugnow)])`.
    2. Set the port and password in the initialization API in the native language being used.
    3. Launch the application and go through the steps to reproduce the issue.

In Python this would look like:
~~~
% From the SWI Prolog top level
?- tdebug, mqi_start([port(4242), password(debugnow)]).
% The graphical front-end will be used for subsequent tracing
true.
~~~
~~~
# Python using the swiplserver library {#mqi-library}
from swiplserver import PrologMQI, PrologThread

with PrologMQI(4242, "debugnow") as mqi:
    with mqi.create_thread() as prolog_thread:
        # Your code to be debugged here
~~~

At this point, all of the multi-threaded debugging tools in SWI Prolog are available for debugging the problem. If the issue is an unexpected exception, the exception debugging features of SWI Prolog can be used to break on the exception and examine the state of the application.  If it is a logic error, breakpoints can be set to halt at the point where the problem appears, etc.

Note that, while using an MQI library to access Prolog will normally end and restart the process between runs of the code, running the server in standalone mode doesn't clear state between launches of the application.  You'll either need to relaunch between runs or build your application so that it does the initialization at startup.

## Machine Query Interface Messages {#mqi-messages}
The messages the Machine Query Interface responds to are described below. A few things are true for all of them:

- Every connection is in its own separate thread. Opening more than one connection means the code is running concurrently.
- Closing the socket without sending `close` and waiting for a response will halt the process if running in ["Embedded Mode"](#mqi-embedded-mode). This is so that stopping a debugger doesn't leave the process orphaned.
- All messages are request/response messages. After sending, there will be exactly one response from the MQI.
- Timeout in all of the commands is in seconds. Sending a variable (e.g. `_`) will use the default timeout passed to the initial `mqi_start/1` predicate and `-1` means no timeout.
- All queries are run in the default module context of `user`. `module/1` has no effect.

### Machine Query Interface Message Format {#mqi-message-format}
Every Machine Query Interface message is a single valid Prolog term. Those that run queries have an argument which represents the query as a single term. To run several goals at once use `(goal1, goal2, ...)` as the goal term.

The format of sent and received messages is identical (`\n` stands for the ASCII newline character which is a single byte):
~~~
<stringByteLength>.\n<stringBytes>.\n.
~~~
For example, to send `hello` as a message you would send this:
~~~
7.\nhello.\n
~~~
 - =|<stringByteLength>|= is the number of bytes of the string to follow (including the =|.\n|=), in human readable numbers, such as `15` for a 15 byte string. It must be followed by =|.\n|=.
 - =|<stringBytes>|= is the actual message string being sent, such as =|run(atom(a), -1).\n|=. It must always end with =|.\n|=. The character encoding used to decode and encode the string is UTF-8.

To send a message to the MQI, send a message using the message format above to the localhost port or Unix Domain Socket that the MQI is listening on.  For example, to run the synchronous goal `atom(a)`, send the following message:
~~~
18.\nrun(atom(a), -1).\n<end of stream>
~~~
You will receive the response below on the receive stream of the same connection you sent on. Note that the answer is in JSON format. If a message takes longer than 2 seconds, there will be "heartbeat" characters (".") at the beginning of the response message, approximately 1 every 2 seconds. So, if the query takes 6 seconds for some reason, there will be three "." characters first:
~~~
...12\ntrue([[]]).\n
~~~

### Machine Query Interface Messages Reference {#mqi-messages}

The full list of Machine Query Interface messages is described below:


- run(Goal, Timeout)

Runs `Goal` on the connection's designated query thread. Stops accepting new commands until the query is finished and it has responded with the results.  If a previous query is still in progress, waits until the previous query finishes (discarding that query's results) before beginning the new query.

Timeout is in seconds and indicates a timeout for generating all results for the query. Sending a variable (e.g. `_`) will use the default timeout passed to the initial `mqi_start/1` predicate and `-1` means no timeout.

While it is waiting for the query to complete, sends a "." character *not* in message format, just as a single character, once every two seconds to proactively ensure that the client is alive. Those should be read and discarded by the client.

If a communication failure happens (during a heartbeat or otherwise), the connection is terminated, the query is aborted and (if running in ["Embedded Mode"](#mqi-embedded-mode)) the SWI Prolog process shuts down.

When completed, sends a response message using the normal message format indicating the result.

Response:

|`true([Answer1, Answer2, ... ])` | The goal succeeded at least once. The response always includes all answers as if run with findall() (see run_async/3 below to get individual results back iteratively).  Each `Answer` is a list of the assignments of free variables in the answer. If there are no free variables, `Answer` is an empty list. |
|`false` | The goal failed. |
|`exception(time_limit_exceeded)` | The query timed out. |
|`exception(Exception)` | An arbitrary exception was not caught while running the goal. |
|`exception(connection_failed)` | The query thread unexpectedly exited. The MQI will no longer be listening after this exception. |

- run_async(Goal, Timeout, Find_All)

Starts a Prolog query specified by `Goal` on the connection's designated query thread. Answers to the query, including exceptions, are retrieved afterwards by sending the `async_result` message (described below). The query can be cancelled by sending the `cancel_async` message. If a previous query is still in progress, waits until that query finishes (discarding that query's results) before responding.

Timeout is in seconds and indicates a timeout for generating all results for the query. Sending a variable (e.g. `_`) will use the default timeout passed to the initial `mqi_start/1` predicate and `-1` means no timeout.

If the socket closes before a response is sent, the connection is terminated, the query is aborted and (if running in ["Embedded Mode"](#mqi-embedded-mode)) the SWI Prolog process shuts down.

If it needs to wait for the previous query to complete, it will send heartbeat messages (see ["Machine Query Interface Message Format"](#mqi-message-format)) while it waits.  After it responds, however, it does not send more heartbeats. This is so that it can begin accepting new commands immediately after responding so the client.

`Find_All == true` means generate one response to an `async_result` message with all of the answers to the query (as in the `run` message above). `Find_All == false` generates a single response to an  `async_result` message per answer.

Response:

|`true([[]])` | The goal was successfully parsed. |
|`exception(Exception)` | An error occurred parsing the goal. |
|`exception(connection_failed)` | The goal thread unexpectedly shut down. The MQI will no longer be listening after this exception. |


- cancel_async
Attempt to cancel a query started by the `run_async` message in a way that allows further queries to be run on this Prolog thread afterwards.

If there is a goal running, injects a `throw(cancel_goal)` into the executing goal to attempt to stop the goal's execution. Begins accepting new commands immediately after responding. Does not inject `abort/0` because this would kill the connection's designated thread and the system is designed to maintain thread local data for the client. This does mean it is a "best effort" cancel since the exception can be caught.

`cancel_async` is guaranteed to either respond with an exception (if there is no query or pending results from the last query), or safely attempt to stop the last executed query even if it has already finished.

To guarantee that a query is cancelled, send `close` and close the socket.

It is not necessary to determine the outcome of `cancel_async` after sending it and receiving a response. Further queries can be immediately run. They will start after the current query stops.

However, if you do need to determine the outcome or determine when the query stops, send `async_result`. Using `Timeout = 0` is recommended since the query might have caught the exception or still be running.  Sending `async_result` will find out the "natural" result of the goal's execution. The "natural" result depends on the particulars of what the code actually did. The response could be:

|`exception(cancel_goal)` | The query was running and did not catch the exception. I.e. the goal was successfully cancelled. |
|`exception(time_limit_exceeded)` | The query timed out before getting cancelled. |
|`exception(Exception)` | They query hits another exception before it has a chance to be cancelled. |
| A valid answer | The query finished before being cancelled. |

Note that you will need to continue sending `async_result` until you receive an `exception(Exception)` message if you want to be sure the query is finished (see documentation for `async_result`).

Response:

| `true([[]])` | There is a query running or there are pending results for the last query. |
| `exception(no_query)` | There is no query or pending results from a query to cancel. |
| `exception(connection_failed)` | The connection has been unexpectedly shut down. The MQI will no longer be listening after this exception. |


- async_result(Timeout)
Get results from a query that was started via a `run_async` message. Used to get results for all cases: if the query terminates normally, is cancelled by sending a `cancel_async` message, or times out.

Each response to an `async_result` message responds with one result and, when there are no more results, responds with `exception(no_more_results)` or whatever exception stopped the query. Receiving any `exception` response except `exception(result_not_available)` means there are no more results. If `run_async` was run with `Find_All == false`, multiple `async_result` messages may be required before receiving the final exception.

Waits `Timeout` seconds for a result. `Timeout == -1` or sending a variable for Timeout indicates no timeout. If the timeout is exceeded and no results are ready, sends `exception(result_not_available)`.

Some examples:

|If the query succeeds with N answers...                             | `async_result` messages 1 to N will receive each answer, in order,  and `async_result` message N+1 will receive `exception(no_more_results)` |
|If the query fails (i.e. has no answers)...                         | `async_result` message 1 will receive `false` and `async_result` message 2 will receive `exception(no_more_results)` |
|If the query times out after one answer...                          | `async_result` message 1 will receive the first answer and `async_result` message 2 will receive `exception(time_limit_exceeded)` |
|If the query is cancelled after it had a chance to get 3 answers... | `async_result` messages 1 to 3 will receive each answer, in order,  and `async_result` message 4 will receive `exception(cancel_goal)` |
|If the query throws an exception before returning any results...    | `async_result` message 1 will receive `exception(Exception)`|

Note that, after sending `cancel_async`, calling `async_result` will return the "natural" result of the goal's execution. The "natural" result depends on the particulars of what the code actually did since this is multi-threaded and there are race conditions. This is described more below in the response section and above in `cancel_async`.

Response:

|`true([Answer1, Answer2, ... ])` | The next answer from the query is a successful answer. Whether there are more than one `Answer` in the response depends on the `findall` setting. Each `Answer` is a list of the assignments of free variables in the answer. If there are no free variables, `Answer` is an empty list.|
|`false`| The query failed with no answers.|
|`exception(no_query)` | There is no query in progress.|
|`exception(result_not_available)` | There is a running query and no results were available in `Timeout` seconds.|
|`exception(no_more_results)` | There are no more answers and no other exception occurred. |
|`exception(cancel_goal)`| The next answer is an exception caused by `cancel_async`. Indicates no more answers. |
|`exception(time_limit_exceeded)`| The query timed out generating the next answer (possibly in a race condition before getting cancelled).  Indicates no more answers. |
|`exception(Exception)`| The next answer is an arbitrary exception. This can happen after `cancel_async` if the `cancel_async` exception is caught or the code hits another exception first.  Indicates no more answers. |
|`exception(connection_failed)`| The goal thread unexpectedly exited. The MQI will no longer be listening after this exception.|


- close
Closes a connection cleanly, indicating that the subsequent socket close is not a connection failure. Thus it doesn't shutdown the MQI in ["Embedded Mode"](#mqi-embedded-mode).  The response must be processed by the client before closing the socket or it will be interpreted as a connection failure.

Any asynchronous query that is still running will be halted by using `abort/0` in the connection's query thread.

Response:
`true([[]])`


- quit
Stops the MQI and ends the SWI Prolog process. This allows client language libraries to ask for an orderly shutdown of the Prolog process.

Response:
`true([[]])`
