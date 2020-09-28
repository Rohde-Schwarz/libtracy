/* 
 * Phantom header for disabling Tracy via compiler optimization in case of
 * building for Release.
 */
#ifndef _tracy_head
#define _tracy_head

#include <stdio.h> /* necessary for size_t */
#include <stdbool.h>
#include <stdarg.h>

/* You may change this constant */
#define TRACY_MAX_SBMTPRNT_LEN 256

/* DO NOT CHANGE these constants. They can only be changed in lib.rs */
#define TRACY_MAX_TRPT_NAME_LEN 32 /* Excluding terminating 0 */
#define TRACY_MAX_SUBMIT_LEN = 2048

#define TRACY_MCAST_DEFAULT_ADDR_V4 "224.0.0.1:64042"
#define TRACY_MCAST_DEFAULT_ADDR_V6 "[ff02::1]:64042"

static inline void* tracy_init(const char *hostname,
				  const char *process_name,
				  unsigned buffer_flush_interval,
				  unsigned announce_interval,
				  const char *announce_iface,
				  const char *announce_mcast_addr,
				  int flags)
{
	(void)hostname;
	(void)process_name;
	(void)buffer_flush_interval;
	(void)announce_interval;
	(void)announce_iface;
	(void)announce_mcast_addr;
	(void)flags;

	return NULL;
}


static inline void tracy_finit(void *tracer)
{
	(void)tracer;

	return;
}


static inline int tracy_register(void *tracer, const char *tracepoint_name)
{
	(void)tracer;
	(void)tracepoint_name;

	return 0;
}


static inline bool tracy_tracepoint_enabled(void *tracer,
		const char *tracepoint_name)
{
	(void)tracer;
	(void)tracepoint_name;

	return false;
}


static inline void tracy_submit(void *tracer, const char *tracepoint_name,
		const void *data, size_t data_len)
{
	(void)tracer;
	(void)tracepoint_name;
	(void)data;
	(void)data_len;

	return;
}


static inline void tracy_submit_printf(void *tracer, const char *tracepoint_name,
		const char *fmt, ...)
{
	(void)tracer;
	(void)tracepoint_name;
	(void)fmt;

	return;
}

#endif
