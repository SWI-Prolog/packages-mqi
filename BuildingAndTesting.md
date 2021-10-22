# How to test the `swiplserver` Module in Python
1. Create a new directory
2. Copy test_prologserver.py into the directory
3. Open a command window
2. cd to the directory you created and:
~~~
python3 -m venv ./env
source env/bin/activate
pip install swiplserver
python test_prologserver.py
~~~
 
# How to build the `swiplserver` module so that `pip install swiplserver` works
First do a build.  This will build into a directory called `/repository_root/python/dist` using the `pyproject.toml` and `setup.cfg` files (and the files they reference) in the `/repository_root/python/` subdirectory.  

Make sure to increase the version number in `setup.cfg` before you build:
~~~
cd /repository_root/python
python3 -m venv ./env
source env/bin/activate
pip install build
pip install twine
python3 -m build
~~~
Then, upload for testing in the "test Python Package Index": https://test.pypi.org/. You'll need to create an account there and get an API token to do this test.  

You will be prompted for a username and password. For the username, use `__token__`. For the password, use the API token value, including the pypi- prefix.
~~~
python3 -m twine upload --repository testpypi dist/*
~~~

Or to upload for release (you'll need an account on http://www.pypi.org to do this):
~~~
python3 -m twine upload dist/*
~~~

# Building documentation

## How to build the Python documentation
The Python documentation is hosted on https://www.swi-prolog.org/packages/mqi/prologmqi.html.  It can be updated by updating the docs at: https://github.com/SWI-Prolog/plweb-www/tree/master/packages/mqi.

HTML Docs produced with https://pdoc3.github.io like this:

~~~
cd /repository_root/python
python3 -m venv ./env
source env/bin/activate
pip install pdoc3
pdoc --html --force --output-dir docs --config show_source_code=False swiplserver.prologmqi
~~~

## How to build the Prolog documentation
The SWI Prolog documentation is automatically built from the sources and hosted at: https://www.swi-prolog.org/pldoc/doc_for?object=section(%27packages/mqi.html%27).  To update the docs, simply change the sources in this respository.

If you want to build them locally for some reason, run the following from the SWI Prolog top level:
~~~
consult("/.../swiplserver/mqi/mqi.pl").
doc_save("/.../swiplserver/mqi/mqi.pl", [doc_root("/.../swiplserver/docs/mqi")]).
consult("/.../swiplserver/mqi/mqi_overview_doc.pl").
doc_save("/.../swiplserver/mqi/mqi_overview_doc.pl", [doc_root("/.../swiplserver/docs/mqi")]).
~~~
