
# swiplserver

> Note that swiplserver 1.0 changes the names of classes from previous versions in an incompatible way -- the terminology changed from 'language server' to 'machine query interface (MQI)'.  You'll need to update your code as you transition.  The names should be stable from version 1.0 on, however.

The `swiplserver` module provides a set of classes to call SWI Prolog from Python. It allows running any query from Python that could be executed from the SWI Prolog console (i.e. the "top level"). Answers to Prolog queries are returned as JSON.

The library uses a SWI Prolog interface called the [Machine Query Interface ('MQI')](https://www.swi-prolog.org/pldoc/doc_for?object=section(%27packages/mqi.html%27)) that allows Prolog queries to be executed. It also manages launching and shutting down SWI Prolog automatically, making the process management invisible to the developer.  The whole experience should feel just like using any other library.

~~~
from swiplserver import PrologMQI, PrologThread

with PrologMQI() as mqi:
    with mqi.create_thread() as prolog_thread:
        result = prolog_thread.query("member(X, [color(blue), color(red)])")
        print(result)

[{'X': {'functor': 'color', 'args': ['blue']}},
 {'X': {'functor': 'color', 'args': ['red']}}]
~~~

To install and learn how to use the swiplserver Python library, see [the docs](https://www.swi-prolog.org/packages/mqi/prologmqi.html).


### Supported Configurations
Should work on:
- MQI protocol version 1.x or prior
- SWI Prolog 8.2.2 or greater (may work on older builds, untested)
- Any Mac, Linux Variants or Windows that are supported by SWI Prolog
- Python 3.7 or later (may work on older builds, untested)

Has been tested with:
- Ubuntu 20.04.2 + SWI Prolog 8.3.22 + Python 3.7.8
- Windows 10 Pro 64 bit + SWI Prolog 8.3.27 + Python 3.7.0
- Windows 8.1 Pro 64 bit + SWI Prolog 8.2.4 + Python 3.8.1
- MacOS Catalina/Big Sur + SWI Prolog 8.3.24 + Python 3.7.4

### Performance
If you're interested in rough performance overhead of the approach this library takes.  On a late 2013 macbook pro the per call overhead of the library for running a Prolog query is about:
- 170 uSec per call using TCP/IP localhost
- 145 uSec per call using Unix Domain Sockets
