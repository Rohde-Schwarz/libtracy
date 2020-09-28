#! /usr/bin/env python3.7

#
# Copyright 2019, 2020 Rohde & Schwarz GmbH & Co KG
# 	philipp.stanner@rohde-schwarz.com
# 	hagen.pfeifer@rohde-schwarz.com
#
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
#
# ----------------------------------------------------------------------------
#
# A very minimalistic, uncomplete tracy client.
# 


import asyncio
import types
from datetime import datetime
import time

TRACEPOINT_LIST_REQUEST = int(1).to_bytes(2, 'big')
TRACEPOINT_LIST_REPLY = int(2).to_bytes(2, 'big')
TRACEPOINT_ENABLE_REQUEST = int(3).to_bytes(2, 'big')
TRACEPOINT_DISABLE_REQUEST = int(4).to_bytes(2, 'big')
TRACE_PUSH = int(5).to_bytes(2, 'big')
MAGIC_NO = bytearray('RuSt'.encode('utf-8'))


class Tracy(asyncio.Protocol):
    def __init__(self, on_con_lost, loop):
        self.loop = loop
        self.on_con_lost = on_con_lost
        self.tracepoints = []
        rec_messages = []
        self.all_tracepoints_enabled = False
        self.print_calls = 0

    def generate_header(self, cmd, msg_len):
        return MAGIC_NO + cmd + msg_len.to_bytes(4, 'big')

    def parse_header(self, head, total_len):
        magic = head[0:4]
        flags = head[4:6]
        cmd = head[6:8]
        rec_len = int.from_bytes(head[8:], 'big')

        if magic != MAGIC_NO:
            print("Magic Number " + str(magic) + " invalid.")
            return (None, 0)

        if total_len != rec_len + 12:
            return (None, 0)

        if cmd == TRACE_PUSH or cmd == TRACEPOINT_LIST_REPLY:
            return (cmd, rec_len)
        else:
            return (None, 0)

    def generate_enable_msg(self, tracepoints):
        total_len = 0
        for tp in tracepoints:
            total_len += len(tp)

        msg = self.generate_header(TRACEPOINT_ENABLE_REQUEST, total_len)

        for tracepoint in tracepoints:
            msg += self.generate_tlv_submsg(tracepoint.lower())

        return msg

    def generate_tlv_submsg(self, msg):
        sublen = len(msg)

        tlv = sublen.to_bytes(2, 'big')
        tlv += bytes(msg, 'ascii')
        return tlv

    def req_tracepoint_list(self):
        msg = self.generate_tracepoint_list_request_msg()
        self.transport.write(msg)

    def generate_tracepoint_list_request_msg(self):
        return self.generate_header(TRACEPOINT_LIST_REQUEST, 0)

    def enable_tracepoints(self, transport, tracepoints):
        msg = self.generate_enable_msg(tracepoints)
        transport.write(msg)
        self.all_tracepoints_enabled = True

    def print_payload(self, message):
        name = message.name.decode('ascii')
        tstamp = int.from_bytes(message.timestamp, 'big')
        print("Received Message(s):")
        print('TP: ' + name + ' | with timestamp: ' +
                self.pretty_timestamp(message.timestamp) + '\n\t' +
                'payload: ' + str(message.payload))

    def pretty_timestamp(self, tstamp):
        timestamp = int.from_bytes(tstamp, 'big')

        # TODO: Do we want to see UTC or localtime?
        date = datetime.fromtimestamp(timestamp // 1e9)
        s = "SystemTime (here): {:d}:{:02d}:{:02d}".format(date.hour, date.minute,
                date.second)
        s += '.' + str(int(timestamp % int(1e9))).zfill(6)
        return s

    # Currently we're assuming that parse_data is only ever called when a
    # TRACE_PUSH arrives
    def parse_data(self, data):
        offset = 0
        while offset < len(data):
            header = data[offset:offset + 12]

            # Get command number. None if header was invalid
            cmd, tracer_msg_len = self.parse_header(header, len(data))
            offset += 12
            if cmd is None:
                break

            if cmd == TRACE_PUSH:
                offset = self.parse_trace_push_msg(data, tracer_msg_len, offset)
            elif cmd == TRACEPOINT_LIST_REPLY:
                offset = self.parse_tracepoint_list_msg(data, tracer_msg_len,
                        offset)

    def parse_trace_push_msg(self, data, tracer_msg_len, offset):
        parsed = 0
        old_offset = offset

        while parsed < tracer_msg_len:
            message = types.SimpleNamespace()
            tp_name_len = self.sub_msg_len(data, offset)
            offset += 2

            message.name = data[offset:offset + tp_name_len]
            offset += tp_name_len

            # TODO: Parse timestamp into other data structure
            message.timestamp = data[offset:offset + 8]
            offset += 8

            data_len = self.sub_msg_len(data, offset)
            offset += 2
            message.payload = data[offset:offset + data_len]
            offset += data_len

            self.rec_messages.append(message)

            parsed += (offset - old_offset)
            old_offset = offset

        return offset

    def parse_tracepoint_list_msg(self, data, tracer_msg_len, offset):
        parsed = 0
        old_offset = offset

        while parsed < tracer_msg_len:
            tp_name_len = self.sub_msg_len(data, offset)
            offset += 2

            tracepoint = data[offset:offset + tp_name_len]
            offset += tp_name_len

            tracepoint = tracepoint.decode('ascii')
            self.tracepoints.append(tracepoint)

            parsed += (offset - old_offset)
            old_offset = offset

        # We received new tracepoints, so maybe not all are enabled anymore
        self.all_tracepoints_enabled = False
        return offset

    def sub_msg_len(self, data, offset):
        return int.from_bytes(data[offset:offset+2], 'big')

    def print_all_messages_in_buf(self):
        self.print_calls += 1
        print("Msgs in rec-buf before printing: " + str(len(self.rec_messages)))
        for msg in self.rec_messages:
            self.print_payload(msg)
        self.rec_messages = []
        print("Msgs in rec-buf after printing: " + str(len(self.rec_messages)))

    def data_received(self, data):
        # time.sleep(100)
        self.parse_data(data)
        self.print_all_messages_in_buf()

        # If we got a tracepoint list during the last receive, activate them all
        if not self.all_tracepoints_enabled:
            self.enable_tracepoints(self.transport, self.tracepoints)

    def connection_lost(self, exc):
        print('The Tracer closed the connection')
        self.on_con_lost.set_result(True)

    def connection_made(self, transport):
        self.transport = transport
        self.req_tracepoint_list()
        self.rec_messages = []


async def main():
    # Get a reference to the event loop as we plan to use
    # low-level APIs.
    loop = asyncio.get_running_loop()

    on_con_lost = loop.create_future()

    transport, protocol = await loop.create_connection(
        lambda: Tracy(on_con_lost, loop),
        'localhost', 61455)

    # Wait until the protocol signals that the connection
    # is lost and close the transport.
    try:
        await on_con_lost
    finally:
        transport.close()


asyncio.run(main())
