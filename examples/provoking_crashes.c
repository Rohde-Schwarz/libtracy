/*
 * Copyright 2019, 2020 Rohde & Schwarz GmbH & Co KG
 * 	philipp.stanner@rohde-schwarz.com
 * 	hagen.pfeifer@rohde-schwarz.com
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * ATTENTION
 * This code example only exists to demonstrate to you how you can kill Tracy
 * and your own application, although the tracer is written in Rust
 */

#include "tracy.h"
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

int main(int argc, char *argv[])
{
	(void)argc;
	char *invalid_tp = "servus";
	void *tracer = tracy_init("Best-Radio", argv[0], 1000, 5000,
			"127.0.0.1", TRACY_MCAST_DEFAULT_ADDR_V4, 0);
	/* You should check for error here... */

	/* If "tracer" is invalid and not NULL, the program will crash here */
	tracy_register(tracer, invalid_tp);

	/* If it did not crash, we can provoke a crash by handing
	 * over ridiculous parameters which the interface function can not check
	 * for validity */
	char *attack = 0x1234567890123456;
	tracy_submit(tracer, attack, "die!", 42);
	/* The interface function surely has killed the whole application
	 * by now, failed dereferencing in its unsafe-blocks */

	tracy_finit(tracer);

	return EXIT_SUCCESS;
}
