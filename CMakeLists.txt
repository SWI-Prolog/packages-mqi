cmake_minimum_required(VERSION 3.5)
project(swipl-language_server)

include("../cmake/PrologPackage.cmake")

swipl_plugin(language_server
	     PL_LIBS language_server.pl)

pkg_doc(language_server
        SECTION
            language_server_overview.md
            SOURCE language_server.pl language_serverpl.tex)

install(DIRECTORY ${CMAKE_SOURCE_DIR}/packages/language_server/python/swiplserver
        DESTINATION ${SWIPL_INSTALL_PREFIX}/lib)

test_libs(language_server
	  PACKAGES plunit)

if(INSTALL_TESTS)
  install(DIRECTORY python
	  DESTINATION ${INSTALL_TESTS_DIR}/packages/language_server
	  FILES_MATCHING PATTERN "*.py")
endif()
