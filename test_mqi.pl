/*  Prolog Language Server
    Author:        Eric Zinda
    E-mail:        ericz@inductorsoftware.com
    WWW:           http://www.inductorsoftware.com
    Copyright (c)  2021-2023, Eric Zinda
                              SWI-Prolog Solutions b.v.
    All rights reserved.

    Redistribution and use in source and binary forms, with or without
    modification, are permitted provided that the following conditions
    are met:

    1. Redistributions of source code must retain the above copyright
       notice, this list of conditions and the following disclaimer.

    2. Redistributions in binary form must reproduce the above copyright
       notice, this list of conditions and the following disclaimer in
       the documentation and/or other materials provided with the
       distribution.

    THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
    "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
    LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS
    FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
    COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT,
    INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
    BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
    LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
    CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
    LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN
    ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
    POSSIBILITY OF SUCH DAMAGE.
*/

:- module(test_mqi,
          [ test_mqi/0,
            test_mqi_all/0
          ]).
:- use_module(library(plunit)).
:- use_module(library(process)).
:- use_module(library(debug)).
:- use_module(library(mqi)).

:- debug(test).

:- dynamic
    python_exe/1.

has_python :-
    python_exe(_),
    !.
has_python :-
    has_python(Prog),
    asserta(python_exe(Prog)).

has_python(Prog) :-
    exe_options(Options),
    absolute_file_name(path(python), Prog, Options).

exe_options(Options) :-
    current_prolog_flag(windows, true),
    !,
    (   Options = [ extensions(['',exe,com]), access(read), file_errors(fail) ]
    ;   Options = [ extensions(['',exe,com]), access(exist), file_errors(fail) ]
    ).
exe_options(Options) :-
    Options = [ access(execute) ].


test_mqi :-
    (   has_python
    ->  run_tests([py_mqi_fast])
    ;   print_message(informational, test_no_python)
    ).
test_mqi_all :-
    (   has_python
    ->  run_tests([py_mqi])
    ;   print_message(informational, test_no_python)
    ).

% Launch the python script with command line arguments so it can, in turn,
% launch the proper development build of prolog, passing all the same command
% line arguments to it
run_test_script(Script, Status, EssentialOnly) :-
    source_file(test_mqi, ThisFile),
    file_directory_name(ThisFile, ThisDir),
    current_prolog_flag(os_argv, [_|Args]),
    current_prolog_flag(executable, Swipl_exe),
    absolute_file_name(Swipl_exe, Swipl),
    file_directory_name(Swipl, Swipl_Path),
    atomic_list_concat(Args, '~|~', Args_String),
    debug(test, 'swipl in dir ~p; Packed args: ~p', [Swipl_Path, Args_String]),
    % Python for Windows wants this
    (   current_prolog_flag(windows, true)
    ->  getenv('SYSTEMROOT', SR),
        System_Root = ['SYSTEMROOT'=SR]
    ;   System_Root = []
    ),
    python_exe(Python),
    process_create(Python, [Script],
                   [ stdin(std),
                     stdout(pipe(Out)),
                     stderr(pipe(Out)),
                     process(PID),
                     cwd(ThisDir),
                     environment([ 'PROLOG_PATH'=Swipl_Path,
                                   'PROLOG_ARGS'=Args_String,
                                   'ESSENTIAL_TESTS_ONLY'=EssentialOnly
                                   | System_Root
                                 ])]),
    (   debugging(test)
    ->  call_cleanup(copy_stream_data(Out, current_output),
                     close(Out))
    ;   setup_call_cleanup(
            open_null_stream(Null),
            copy_stream_data(Out, Null),
            close(Null))
    ),
    process_wait(PID, Status).

:- begin_tests(py_mqi_fast, [sto(rational_trees)]).

test(mqi, Status == exit(0)):-
    run_test_script('python/test_prologserver.py', Status, 'True').

:- end_tests(py_mqi_fast).

:- begin_tests(py_mqi, [sto(rational_trees)]).

test(mqi, Status == exit(0)):-
    run_test_script('python/test_prologserver.py', Status, 'False').

:- end_tests(py_mqi).


		 /*******************************
		 *           MESSAGES		*
		 *******************************/

:- multifile prolog:message//1.

prolog:message(test_no_python) -->
    [ 'Could not find Python.  Skipping MQI tests.' ].
