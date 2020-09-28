/*
 * Copyright 2019, 2020 Rohde & Schwarz GmbH & Co KG
 * 	philipp.stanner@rohde-schwarz.com
 * 	hagen.pfeifer@rohde-schwarz.com
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * ----------------------------------------------------------------------------
 *
 * Tracing Library written in Rust, with C interface described below.
 * The tracer allows you to register so called tracepoints to which you can
 * submit data. This data will be transmitted automatically to a connected
 * client over the network, without your involvement.
 *
 * All functions check if NULL-Pointers are transfered, but, just like C, Rust
 * can not check if transfered references are valid. Unproper usage therfore
 * may lead to various faults.
 *
 * Note that none of the following interface functions ever changes data pointed
 * to by parameter pointers. The functions always create deep copies to work on.
 */
#ifndef _tracy_head
#define _tracy_head

#include <stdio.h> /* necessary for size_t */
#include <stdbool.h>
#include <stdarg.h>

/* You may change this constant */
#define TRACY_MAX_SBMTPRNT_LEN 256

/* DO NOT CHANGE these two constants. They can only be changed in lib.rs */
#define TRACY_MAX_TRPT_NAME_LEN 32 /* Excluding terminating 0 */
#define TRACY_MAX_SUBMIT_LEN = 2048

#define TRACY_MCAST_DEFAULT_ADDR_V4 "225.0.0.1:64042"
#define TRACY_MCAST_DEFAULT_ADDR_V6 "[ff02::4242:beef:1]:64042"


/*
 * Spawns a new thread, which will administrate the tracing-services. It
 * announces host- and process-name and the port number of a TCP socket via
 * UDP-multicasts over the network. Clients then can connect to the TCP socket.
 *
 * One call to tracy_init creates one thread, and this one thread accepts
 * exactly one TCP connection.
 *
 * Returns an opaque pointer, containing all relevant data for the other
 * interface functions. This pointer and the data it references must not be
 * changed in any way by the calling application!
 *
 * You have to provide valid values for all parameters for init to work,
 * with the only exceptions being:
 * 		- announce_interval: If you set it to 0, the tracer will not announce
 * 			its presence via UDP mcasts
 * 		- announce_iface: If you set it to NULL, the tracer will not announce
 * 			its presence via UDP mcasts 
 * 		- announce_mcast_addr: If you set it to NULL, the tracer will not
 * 			announce its presence via UDP mcasts
 *
 * You have to specify the interface on which UDP announcements are send via
 * the IFs IP addr:
 * announce_iface = "192.168.0.1";
 * For IPv6 addresses, you additionally have to specify the interface:
 * announce_iface = "fe80::5ef9:ddff:fe74:496b%eno1"
 *
 * For UDP-mcast-addresses, it is highly recommended to use the default mcast
 * addresses defined above in this header. Idiomatic clients will search on
 * these addresses for targets.
 * Naturally, you have to either use the IPv4 or IPv6 default address matching
 * to the type of interface address you specified.
 * If you want to specify a target address manually, you have to set the port
 * number to 0:
 * "192.168.0.1:0"
 * "[fe80::5ef9:ddff:fe74:496b]:0" (interface not necessary in this case)
 *
 * 
 * The unsigneds, specifying time-constants, are treated by Tracy as milliseconds.
 *
 * If either hostname or process_name are NULL or if announce_interval is 0,
 * init will return NULL and ignore your request.
 *
 * int flags is reserved for later functionality extensions. For this version,
 * you can always set flags to 0.
 */
void* tracy_init(const char *hostname,
                  const char *process_name,
                  unsigned buffer_flush_interval, /* as milliseconds */
                  unsigned announce_interval, /* as milliseconds */
                  const char *announce_iface,
                  const char *announce_mcast_addr,
                  int flags);


/*
 * Terminates the whole tracer described by *tracy and its thread, 
 * closes all sockets, deallocates all used memory.
 *
 * The thread spawned by tracy_init may keep running for a short period of
 * time after tracy_finit has returned.
 *
 * IMPORTANT:
 * tracy_finit frees the memory occupied by *tracer. You therefore MUST
 * NOT call free(tracer), neither before nor after, otherwise behavior (of the
 * libc function free() ) is undefined or you'll create a zombie thread.
 * It is encouraged to set *tracer to NULL after tracy_finit has returned.
 */
void tracy_finit(void *tracer);


/*
 * Register a new tracepoint which can be activated by the client over TCP.
 * Once the tracepoint got activated, tracy_submit() will accept data
 * submitted by the caller.
 *
 * tracepoint_name should not exceed a length of TRACY_MAX_TRPT_NAME_LEN
 * bytes (excluding terminating null character), otherwise tracy_register()
 * will ignore the additional characters and limit its copy of the string to
 * TRACY_MAX_TRPT_NAME_LEN valid characters.
 *
 * tracepoint_name must consist exclusively of valid 7-Bit-ASCII-characters! If
 * it does not, tracy_register will ignore your register-request.
 *
 * tracepoint names should be all-lowercase only. Uppercase letters will
 * automatically be changed to all-lowercase within the tracer.
 *
 * The function returns 0 on success and a negative number in case of failure,
 * for example when you hand over an invalid string.
 */ 
int tracy_register(void *tracer, const char *tracepoint_name);


/*
 * Tracepoints can be enabled or disabled. Data submitted to the tracer will
 * only be accepted if the tracepoint you're submitting to was enabled.
 *
 * If preparing data for submitting is expensive, this function can be used to
 * check if a certain tracepoint is already enabled.
 *
 * Return value indicates state of tracepoint.
 *
 * The function will return false if you hand over a string which contains at
 * least one non-7-Bit-ASCII-character. If you hand over a string containing
 * uppercase letters, the function will treat it as all-lowercase.
 *
 * If you hand over strings which are longer than TRACY_MAX_TRPT_NAME_LEN,
 * the function will truncate the string to this length internally.
 */
bool tracy_tracepoint_enabled(void *tracer, const char *tracepoint_name);


/*
 * Submits data, referenced by *data, to the tracer-thread, which sends the
 * data to a client, if one is connected and activated the tracepoint. You
 * are only allowed to submit a data amount up to TRACY_MAX_SUBMIT_LEN Bytes.
 *
 * tracy_submit checks if tracepoint_name is a valid 7-Bit-ASCII-String. If
 * it is not, submit executes an early return. If the string contains uppercase
 * letters, submit converts its copy to an all-lowercase string and looks if a
 * tracepoint matching this string has been registered already.
 * 
 * submit returns as soon as possible without processing data if:
 * 	1. A parameter is NULL
 * 	2. data_len is 0 or larger than TRACY_MAX_SUBMIT_LEN
 *	3. No client is connected
 * 	4. tracepoint_name is not valid ASCII
 *	5. The tracepoint has not been registered, yet
 *	6. The client is connected, but has not activated the tracepoint
 *
 * If a client enabled the tracepoint, tracy_submit copies the data
 * immediately after being called. Therefore, after the function returns, you
 * can call free(data) if you wish.
 *
 * The function adds a timestamp (in nanoseconds as UNIX_EPOCH, meaning ns since
 * UTC 01.01.1970 00:00) to the submitted data.
 *
 * Submitted data is buffered for an amount of time specified in the Rust code.
 * Therefore, calling tracy_submit will not make the tracer write data to a
 * network socket immediately, even if the corresponding tracepoint has been
 * activated.
 * All data is treated as plain bytes and therefore, data can be anything,
 * stack-array, heap based, ASCII, JSON etc.
 *
 * tracy_submit checks for NULL-pointers, but can not check the validity of
 * *data and especially data_len has to be exactly the number of bytes the
 * submit function can savely copy by dereferencing *data
 */
void tracy_submit(void *tracer, const char *tracepoint_name,
                  const void *data, size_t data_len);


/*
 * A handy wrapper function for tracy_submit.
 * tracy_submit_printf submits a formatted string to a client. The string
 * formatting works exactly as it's known from printf.
 *
 * If one of the first three parameters is NULL, the function returns early.
 * Beside that, the function behaves like tracy_submit.
 */
static void tracy_submit_printf(void *tracer, const char *tracepoint_name,
                        const char *fmt, ...)
{
	char buffer[TRACY_MAX_SBMTPRNT_LEN];
	int ret;
	va_list ap;
	if (!tracer || !tracepoint_name || !fmt)
		return;

	va_start(ap, fmt);
	ret = vsnprintf(buffer, TRACY_MAX_SBMTPRNT_LEN, fmt, ap);
	va_end(ap);

	if (ret < 0) {
		fprintf(stderr, "tracy_submit_print: Could not write to buffer.\n");
		return;
	}

	tracy_submit(tracer, tracepoint_name, buffer, (size_t)ret);
}


/*
 * Short usage example. For more detailed examples see tracy/examples

 -------------------------

#include "tracy.h"
#include <string.h>
#include <stdlib.h>

int main(void)
{
	char *payload = "Hello world!";
	char *tracepoint = "my_tracepoint";

	---We want to announce every 5 seconds and latest send after 1 second---
	void *tracer = tracy_init("Best-Radio", argv[0], 1000, 5000,
			"127.0.0.1", TRACY_MCAST_DEFAULT_ADDR_V4, 0);

	if (!tracer)
		return EXIT_FAILURE;

	if (tracy_register(tracer, tracepoint) != 0) {
		tracy_finit(tracer);
		return EXIT_FAILURE;
	}

	---If the client is not connected yet, submit will ignore the request---
	tracy_submit(tracer, tracepoint, payload, strlen(payload));

	---Before exiting, the tracer-thread will try to send all data currently in
		the buffer. In this example, though, the client won't have enough time
		to connect---
	tracy_finit(tracer);
	tracer = NULL;

	return EXIT_SUCCESS;
}

 -------------------------
*/

#endif
