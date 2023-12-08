cmake_minimum_required(VERSION 3.5)
project(swipl-mqi)

include("../cmake/PrologPackage.cmake")

swipl_plugin(mqi
	     PL_LIBS mqi.pl)

pkg_doc(mqi
        SECTION
            mqi_overview.md
            SOURCE mqi.pl mqipl.tex)

install(DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/python/swiplserver
        DESTINATION ${SWIPL_INSTALL_PREFIX}/lib)
install_src(mqi_app
	    FILES mqi_app.pl
	    RENAME mqi.pl
	    DESTINATION ${SWIPL_INSTALL_APP})

find_program(PROG_PYTHON NAMES python3)

if(PROG_PYTHON)
  test_libs(mqi
	    PACKAGES plunit)
endif()

if(INSTALL_TESTS)
  install(DIRECTORY python
	  DESTINATION ${INSTALL_TESTS_DIR}/packages/mqi
	  FILES_MATCHING PATTERN "*.py")
endif()
