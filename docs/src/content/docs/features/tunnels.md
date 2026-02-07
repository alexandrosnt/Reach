---
title: Port Tunneling
description: Forward ports through SSH connections.
---

Reach supports SSH port tunneling so you can securely access services that are behind firewalls or only listening on private networks.

## Local Port Forwarding

This is the most common type. You bind a port on your local machine, and traffic to that port gets tunneled through SSH to a remote destination.

A typical use case: there's a MySQL server on `10.0.0.5:3306` that's only reachable from your SSH host. You set up a local forward from `localhost:3306` to `10.0.0.5:3306` through your SSH connection. Now you can point your database client at `localhost:3306` and it goes through the tunnel.

## Setting Up a Tunnel

Go to the Tunnels panel in the sidebar and click "New Tunnel". Pick the type (Local Forward), set the local port and remote destination, and select which SSH connection to route it through.

You can start and stop tunnels independently. They stick around until you remove them, but they don't survive an app restart yet. That's on the roadmap.

## What's Happening Under the Hood

Each tunnel creates a TCP listener on your local machine. When something connects to it, Reach opens an SSH channel to the remote destination and relays traffic in both directions. It's the same thing `ssh -L` does, just with a UI.

## What's Not Here Yet

Remote Port Forwarding and Dynamic SOCKS proxy are planned but not implemented. Right now, only Local Forward works.
