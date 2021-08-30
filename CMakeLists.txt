cmake_minimum_required(VERSION 3.5)
project(swipl-mqi)

include("../cmake/PrologPackage.cmake")

swipl_plugin(mqi
	     PL_LIBS mqi.pl)

pkg_doc(mqi
        SECTION
            mqi_overview.md
            SOURCE mqi.pl mqipl.tex)

install(DIRECTORY ${CMAKE_SOURCE_DIR}/packages/mqi/python/swiplserver
        DESTINATION ${SWIPL_INSTALL_PREFIX}/lib)

test_libs(mqi
	  PACKAGES plunit)

if(INSTALL_TESTS)
  install(DIRECTORY python
	  DESTINATION ${INSTALL_TESTS_DIR}/packages/mqi
	  FILES_MATCHING PATTERN "*.py")
endif()
