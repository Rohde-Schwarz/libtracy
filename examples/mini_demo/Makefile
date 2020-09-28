OUTPUTFILE=hello-rust

.PHONY: all

ROOTDIR = $(realpath .)
HEADER_DIR = $(ROOTDIR)/../../src/
SO_DIR = $(ROOTDIR)/../../target/debug/

all: tracer_demo.c
	cp $(SO_DIR)/libtracy.so .
	$(CC) -I $(HEADER_DIR) -L./ -Wall -Wextra -o tracer-demo $< -ltracy

clean:
	rm tracer-demo libtracy.so
