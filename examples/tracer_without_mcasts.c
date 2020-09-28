/*
 * Copyright 2019, 2020 Rohde & Schwarz GmbH & Co KG
 * 	philipp.stanner@rohde-schwarz.com
 * 	hagen.pfeifer@rohde-schwarz.com
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * This code example shows how you properly initialize and configure Tracy,
 * and how you can submit data and later terminate the tracer again.
 */

#include "tracy.h"
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

int main(int argc, char *argv[])
{
	(void)argc;
	/* Initalize tracer and check for error */
	void *tracer = NULL;

	/* Init the tracer:
	 * We want to send every 1000ms. We do not want to multicast and therefore
	 * set everythin related with mcasts to 0 / NULL.
	 * Flags are set to 0. */
	tracer = tracy_init("Best-Radio", argv[0], 1000, 0, NULL, NULL, 0);

	if (tracer == NULL) {
		fprintf(stderr, "Initializing tracer failed.\n");
		return EXIT_FAILURE;
	}

	/* Choose tracepoint names. Only use lowercase and ASCII. */
	char *tp_status = "system_status";
	char *tp_sensor = "thermal_sensor_0";

	/* Registering the tracepoints. */
	tracy_register(tracer, tp_sensor);
	tracy_register(tracer, tp_status);

	/* Tracy can deal with all sorts of payloads */
	char *state_payload = "Everything is fine.";
	int arr_payload[] = {-42, 9001, 403, 25, 70, 37, 8, 8, 5};

	/* Submit data. If the tracepoints have been enabled by the client,
	 * the tracer will copy the data and transmit it over TCP. */
	tracy_submit(tracer, tp_status, state_payload, strlen(state_payload));
	tracy_submit(tracer, tp_sensor, arr_payload, sizeof(arr_payload));

	/* Stopp tracing services. Always set 'tracer' to NULL to avoid later
	 * accidental usage */
	tracy_finit(tracer);
	tracer = NULL;

	return EXIT_SUCCESS;
}
