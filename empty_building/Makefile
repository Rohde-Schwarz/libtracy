OUTPUTFILE=hello-rust

.PHONY: all
release:
	$(CC) -fPIC -c phantomlib.c
	$(CC) -shared phantomlib.o -o libtracy.so
	ar rsv libtracy.a phantomlib.o

dir:
	ls -alF

clean:
	rm phantomlib.o libtracy.so libtracy.a

# install target is not used, install is done entirely
# by ctrl-agent.py
