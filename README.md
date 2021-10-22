# SWI Prolog Machine Query Interface and Python Integration
This package provides the library mqi.pl that enables embedding SWI Prolog into just about any programming language (Python, Go, C#, etc) in a straightforward way. It is designed for scenarios that need to use SWI Prolog as a local implementation detail of another language. Think of it as running SWI Prolog "like a library". It can support any programming language that can launch processes, read their STDOUT pipe, and send and receive JSON over TCP/IP.

A Python 3.x library that uses the MQI to integrate Python with SWI Prolog is included with SWI Prolog. It is called `swiplserver` and is described in `./python/README.md`.

Developers are encouraged to use the SWI Prolog MQI to integrate SWI Prolog with other languages, just as the swiplserver library does for Python. The MQI code is available in this repository at: `/mqi.pl`  Read more in:
- [Machine Query Interface Overview](https://www.swi-prolog.org/pldoc/doc_for?object=section(%27packages/mqi.html%27))
- [Machine Query Interface Predicates Reference](https://www.swi-prolog.org/pldoc/man?section=mqi)

