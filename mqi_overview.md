# Machine Query Interface Overview		{#mqi-overview}
The SWI Prolog Machine Query Interface ('MQI') is designed to enable embedding SWI Prolog into just about any programming language (Python, Go, C#, etc) in a straightforward way. It is designed for scenarios that need to use SWI Prolog as a local implementation detail of another language. Think of it as running SWI Prolog "like a library". It can support any programming language that can launch processes, read their STDOUT pipe, and send and receive JSON over TCP/IP. A Python 3 library is included as a part of SWI Prolog, see [](#mqi-python-installation).

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
A Python 3.x library that integrates Python with SWI Prolog using the Machine Query Interface is included with in the `libs` directory of the SWI Prolog installation. It is also available using =|pip install swiplserver|=. See the [Python swiplserver library documentation](https://blog.inductorsoftware.com/swiplserver/swiplserver/prologserver.html) for more information on how to use and install it from either location.

## Installation Steps for Other Languages {#mqi-language-installation}

In general, to use the Machine Query Interface with any programming language:

    1. Install SWI Prolog itself on the machine the application will run on.
    2. Check if your SWI Prolog version includes the MQI by launching it and typing `?- mqi([]).` If it can't find it, see below for how to install it.
    3. Ensure that the system path includes a path to the `swipl` executable from that installation.
    4. Make sure the application (really the user that launches the application) has permission to launch the SWI Prolog process. Unless your system is unusually locked down, this should be allowed by default.  If not, you'll need to set the appropriate permissions to allow this.
    5. Install (or write!) the library you'll be using to access the MQI in your language of choice.

If your SWI Prolog version doesn't yet include the MQI:
    1. Download the =|mqi.pl|= file from the [GitHub repository](https://github.com/EricZinda/swiplserver/tree/main/mqi).
    2. Open an operating system command prompt and go to the directory where you downloaded =|mqi.pl|=.
    3. Run the command below. On Windows the command prompt must be [run as an administrator](https://www.wikihow.com/Run-Command-Prompt-As-an-Administrator-on-Windows). On Mac or Linux, start the command with `sudo` as in `sudo swipl -s ...`.

~~~
swipl -s mqi.pl -g "mqi:install_to_library('mqi.pl')" -t halt
~~~

## Prolog Language Differences from the Top Level {#mqi-toplevel-differences}

The Machine Query Interface is designed to act like using the ["top level"](quickstart) prompt of SWI Prolog itself (i.e. the "?-" prompt).  If you've built the Prolog part of your application by loading code, running it and debugging it using the normal SWI Prolog top level, integrating it with your native language should be straightforward: simply run the commands you'd normally run on the top level, but now run them using the query APIs provided by the library built for your target language. Those APIs will allow you to send the exact same text to Prolog and they should execute the same way.  Here's an example using the Python =|swiplserver|= library:

~~~
% Prolog Top Level
?- member(X, [first, second, third]).
X = first ;
X = second ;
X = third.
~~~
~~~
# Python using the swiplserver library
from swiplserver import PrologServer, PrologThread

with PrologServer() as server:
    with server.create_thread() as prolog_thread:
        result = prolog_thread.query("member(X, [first, second, third]).")
        print(result)

first
second
third
~~~

While the query functionality of the MQI does run on a thread, it will always be the *same* thread, and, if you use a single connection, it will only allow queries to be run one at a time, just like the top level. Of course, the queries you send can launch threads, just like the top level, so you are not limited to a single threaded application. There are a few differences from the top level, however:

    - Normally, the SWI Prolog top level runs all user code in the context of a built-in module called "user", as does the MQI. However, the top level allows this to be changed using the module/1 predicate. This predicate has no effect when sent to the MQI.
    - Predefined streams like user_input/0 are initially bound to the standard operating system I/O streams (like STDIN) and, since the Prolog process is running invisibly, will obviously not work as expected. Those streams can be changed, however, by issuing commands using system predicates as defined in the SWI Prolog documentation.
    - Every connection to the MQI runs in its own thread, so opening two connections from an application means you are running multithreaded code.

The basic rule to remember is: any predicates designed to interact with or change the default behavior of the top level itself probably won't have any effect.


## Embedded Mode: Integrating the Machine Query Interface Into a New Programming Language {#mqi-embedded-mode}
The most common way to use the Machine Query Interface is to find a library that wraps and exposes it as a native part of another programming language such as the [Python =|swiplserver|= library](#mqi-python-installation). This section describes how to build one if there isn't yet a library for your language.  To do this, you'll need to familiarize yourself with the MQI protocol as described in the `mqi/1` documentation. However, to give an idea of the scope of work required, below is a typical interaction done (invisibly to the user) in the implementation of any programming language library:


     1. Launch the SWI Prolog process using (along with any other options the user requests): =|swipl --quiet -g mqi -t halt -- --write_connection_values=true|=.  To work, the `swipl` Prolog executable will need to be on the path or specified in the command. This launches SWI Prolog, starts the MQI, and writes the chosen port and password to STDOUT.  This way of launching invokes the mqi/0 predicate that turns off the `int` (i.e. Interrupt/SIGINT) signal to Prolog. This is because some languages (such as Python) use that signal during debugging and it would be otherwise passed to the client Prolog process and switch it into the debugger.  See the mqi/0 predicate for more information on other command line options.
     2. Read the SWI Prolog STDOUT to retrieve the TCP/IP port and password. They are sent in that order, delimited by '\n'.

~~~
$ swipl --quiet -g mqi -t halt -- --write_connection_values=true
54501
185786669688147744015809740744888120144
~~~

    Now the server is started. To create a connection:

     3. Use the language's TCP/IP sockets library to open a socket on the specified port of localhost and send the password as a message. Messages to and from the MQI are in the form =|<stringByteLength>.\n<stringBytes>.\n |= where `stringByteLength` includes the =|.\n|= from the string. For example: =|7.\nhello.\n|= More information on the [message format](#mqi-message-format) is below.
     4. Listen on the socket for a response message of `true([[threads(Comm_Thread_ID, Goal_Thread_ID)]])` (which will be in JSON form) indicating successful creation of the connection.  `Comm_Thread_ID` and `Goal_Thread_ID` are the internal Prolog IDs of the two threads that are used for the connection. They are sent solely for monitoring and debugging purposes.

We can try all of this using the Unix tool `netcat` (also available for Windows) to interactively connect to the MQI. In `netcat` hitting `enter` sends =|\n|= which is what the message format requires. The server responses are show indented inline.

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

Continuing with the `netcat` session (the `quit` message isn't shown since the `close` message closes the connection):
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
Note that Unix Domain Sockets can be used instead of a TCP/IP port. How to do this is described in the [Machine Query Interface Options documentation](#mqi-options).

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
- Where appropriate, use similar names and approaches to the [Python library](https://github.com/EricZinda/swiplserver) when designing your language library. This will give familiarity and faster learning for users that use more than one language.
- Use the `debug/1` predicate described in the `mqi/1` documentation to turn on debug tracing. It can really speed up debugging.
- Read the STDOUT and STDERR output of the SWI Prolog process and output them to the debugging console of the native language to help users debug their Prolog application.

## Standalone Mode: Debugging Prolog Code Used in an Application {#mqi-standalone-mode}
When using the Machine Query Interface from another language, debugging the Prolog code itself can often be done by viewing traces from the Prolog native `writeln/1` or `debug/3` predicates. Their output will be shown in the debugger of the native language used.  Sometimes an issue surfaces deep in an application. When this happens, running the application in the native language while setting breakpoints and viewing traces in Prolog itself is often the best debugging approach. Standalone mode is designed for this scenario.

As the MQI is a multithreaded application, debugging the running code requires using the multithreaded debugging features of SWI Prolog as described in the section on ["Debugging Threads"](threaddebug) in the SWI Prolog documentation. A typical flow for Standalone Mode is:

    1. Launch SWI Prolog and call the `mqi/1` predicate specifying a port and password. Use the `tdebug/0` predicate to set all threads to debugging mode like this: `tdebug, mqi([port(4242), password(debugnow)])`.
    2. Set the port and password in the initialization API in the native language being used.
    3. Launch the application and go through the steps to reproduce the issue.

In Python this would look like:
~~~
% From the SWI Prolog top level
?- tdebug, mqi([port(4242), password(debugnow)]).
% The graphical front-end will be used for subsequent tracing
true.
~~~
~~~
# Python using the swiplserver library
from swiplserver import PrologServer, PrologThread

with PrologServer(4242, "debugnow") as server:
    with server.create_thread() as prolog_thread:
        # Your code to be debugged here
~~~

At this point, all of the multi-threaded debugging tools in SWI Prolog are available for debugging the problem. If the issue is an unexpected exception, the exception debugging features of SWI Prolog can be used to break on the exception and examine the state of the application.  If it is a logic error, breakpoints can be set to halt at the point where the problem appears, etc.

Note that, while using an MQI library to access Prolog will normally end and restart the process between runs of the code, running the server in standalone mode doesn't clear state between launches of the application.  You'll either need to relaunch between runs or build your application so that it does the initialization at startup.
