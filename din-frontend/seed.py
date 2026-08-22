#!/usr/bin/env python3
"""Seed the din-node with a small but illustrative lineage network.

Usage: python3 seed.py [http://127.0.0.1:8080]
"""
import json
import sys
import urllib.request

BASE = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:8080"


def post(path, body):
    req = urllib.request.Request(
        BASE + path,
        data=json.dumps(body).encode(),
        headers={"content-type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req) as r:
        return json.loads(r.read())


def get(path):
    with urllib.request.urlopen(BASE + path) as r:
        return json.loads(r.read())


def contribute(agent, schema, content):
    return post(
        "/events",
        {
            "agent_id": agent,
            "event_type": "contribute",
            "payload": {"owner": agent, "schema": schema, "version": 1, "content": content},
        },
    )


for agent in ["alice", "bob", "carol", "dave", "eve"]:
    post("/agents", {"agent_id": agent})

contribute("alice", "raw/v1", "alice-iot-data")
contribute("bob", "raw/v1", "bob-iot-data")
contribute("carol", "raw/v1", "carol-iot-data")

ds = get("/datasets")["datasets"]


def find(owner, schema):
    return next(d["id"] for d in ds if d["owner"] == owner and d["schema"] == schema)


raw_a = find("alice", "raw/v1")
raw_b = find("bob", "raw/v1")
raw_c = find("carol", "raw/v1")

# dave: cleaning workshop, derives from alice + bob
post(
    "/events",
    {
        "agent_id": "dave",
        "event_type": "subscribe",
        "payload": {
            "child": {"owner": "dave", "schema": "clean/v1", "version": 1, "content": "cleaned"},
            "sources": [
                {"parent": raw_a, "price": 100, "alpha": 0.5},
                {"parent": raw_b, "price": 80, "alpha": 0.5},
            ],
            "external_revenue": 0,
        },
    },
)

# refresh dataset registry after dave's derivation
ds = get("/datasets")["datasets"]

# eve: app, derives from dave's cleaned set + carol's raw, real end-consumers pay 500
post(
    "/events",
    {
        "agent_id": "eve",
        "event_type": "subscribe",
        "payload": {
            "child": {"owner": "eve", "schema": "report/v1", "version": 1, "content": "report"},
            "sources": [
                {"parent": find("dave", "clean/v1"), "price": 200, "alpha": 0.6},
                {"parent": raw_c, "price": 150, "alpha": 0.4},
            ],
            "external_revenue": 500,
        },
    },
)

print(json.dumps(get("/datasets"), indent=2))
