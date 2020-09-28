/*
 * Copyright 2019, 2020 Rohde & Schwarz GmbH & Co KG
 * 	philipp.stanner@rohde-schwarz.com
 * 	hagen.pfeifer@rohde-schwarz.com
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * This example shows how you check if a client has actually requested data
 * from a tracepoint, before preparing and transmitting data
 */

#include "tracy.h"
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

#define PLD_LEN 250

int main(int argc, char *argv[])
{
	(void)argc;
	/* Initalize tracer and check for error */
	void *tracer = NULL;

	/* Init the tracer:
	 * We want to send every 1000ms, announce via UDP once every 5000ms,
	 * bind the UDP-announce-socket to localhost for testing and send to the
	 * specified target address. Flags are set to 0. */
	tracer = tracy_init("Best-Radio", argv[0], 1000, 5000,
			"127.0.0.1", TRACY_MCAST_DEFAULT_ADDR_V4, 0);

	if (tracer == NULL) {
		fprintf(stderr, "Initializing tracer failed.\n");
		return EXIT_FAILURE;
	}

	/* Choose tracepoint names */
	char *tp_sensor = "measurements";

	/* Registering the tracepoints. */
	tracy_register(tracer, tp_sensor);

	int *complex_payload = NULL;
	/* Check if a client is listening for data on the TP
	 * before doing expensive data-preparation */
	if (tracy_tracepoint_enabled(tracer, tp_sensor))
		if (complex_payload = malloc(PLD_LEN))
			for (int i=0; i<PLD_LEN; i++)
				complex_payload[i] = i;

	/* submit ignores calls with NULL-pointers.
	 * Alternatively, we could have jumped to free() with a goto-label */
	tracy_submit(tracer, tp_sensor, complex_payload, PLD_LEN);

	if (complex_payload)
		free(complex_payload);

	tracy_finit(tracer);
	tracer = NULL;

	return EXIT_SUCCESS;
}
