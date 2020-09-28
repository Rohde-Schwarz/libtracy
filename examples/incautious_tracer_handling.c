/*
 * Copyright 2019, 2020 Rohde & Schwarz GmbH & Co KG
 * 	philipp.stanner@rohde-schwarz.com
 * 	hagen.pfeifer@rohde-schwarz.com
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * This example shows how you SHOULD NOT call Tracy-Services.
 * This code shows what kind of misbehavior Tracy *can* handle.
 */

#include "tracy.h"
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

int main(int argc, char *argv[])
{
	(void)argc;
	void *tracer = NULL;
	tracer = tracy_init("Best-Radio", argv[0], 1000, 5000,
			"127.0.0.1", TRACY_MCAST_DEFAULT_ADDR_V4, 0);
	if (tracer == NULL) {
		fprintf(stderr, "Initializing tracer failed.\n");
		return EXIT_FAILURE;
	}

	/* Non-ASCII tracepoints are officially forbidden. */
	char *tp_invalid = "Überprüfung";
	char *tp_status = "system_status";

	/* tracy_register will ignore the invalid tracepoint. */
	tracy_register(tracer, tp_invalid);
	tracy_register(tracer, tp_status);

	char *state_payload = "Everything is fine.";

	/* tracy_submit will ignore the first call, because the
	 * tracepoint has not been registered. */
	tracy_submit(tracer, tp_status, state_payload, strlen(state_payload));
	tracy_submit(tracer, tp_invalid, state_payload, strlen(state_payload));

	/* Stopp tracing services. Set 'tracer' to NULL to avoid later
	 * accidental usage */
	tracy_finit(tracer);
	tracer = NULL;

	return EXIT_SUCCESS;
}
