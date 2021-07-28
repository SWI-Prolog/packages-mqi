/*  Prolog Language Server
    Author:        Eric Zinda
    E-mail:        ericz@inductorsoftware.com
    WWW:           http://www.inductorsoftware.com
    Copyright (c)  2021, Eric Zinda
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

:- use_module(library(plunit)).
:- use_module(language_server).

% Set so that the python script can be loaded
:- prolog_load_context(directory, Dir), working_directory(_, Dir).

test_language_server :-
    run_tests([py_language_server]).

% Launch the python script with command line arguments so it can, in turn,
% launch the proper development build of prolog, passing all the same command
% line arguments to it
run_test_script(Script, Status):-
    current_prolog_flag(os_argv, [Swipl | Args]),
    (   is_absolute_file_name(Swipl)
    ->  file_directory_name(Swipl, Swipl_Path)
    ;   Swipl_Path = ''
    ),
    atomic_list_concat(Args, '~|~', Args_String),
    writeln(Args_String),
    writeln(data(Swipl_Path, Args_String)),
    process_create(path(python3), [Script],
        [stdin(std), stdout(pipe(Out)), stderr(pipe(Out)), process(PID), environment(
                ['PROLOG_PATH'=Swipl_Path, 'PROLOG_ARGS' = Args_String, 'ESSENTIAL_TESTS_ONLY'='True']
            )]),
    read_lines(Out, Lines),
    writeln(Lines),
    process_wait(PID, Status).

:- begin_tests(py_language_server, []).

test(language_server):-
    run_test_script('python/test_prologserver.py', Status),
    assertion(Status == exit(0)).

:- end_tests(py_language_server).

read_lines(Out, Lines) :-
        read_line_to_codes(Out, Line1),
        read_lines(Line1, Out, Lines).

read_lines(end_of_file, _, []) :- !.
read_lines(Codes, Out, [Line|Lines]) :-
        atom_codes(Line_Initial, Codes),
        atomic_list_concat([Line_Initial, '\n'], Line),
        read_line_to_codes(Out, Line2),
        read_lines(Line2, Out, Lines).