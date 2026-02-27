---
created: 2026-02-27T17:45:50.487Z
title: Network Printer Discovery (mDNS/SSDP)
area: general
files: []
---

## Problem

Print farm tools and GUI applications need to discover printers on the local network. This is a common need for any application building on the slicing library that wants to send G-code directly to printers. Protocols involved are mDNS (Bonjour) for OctoPrint/Klipper and SSDP for some networked printers.

## Solution

This is solidly application-layer functionality — a consumer can use the `mdns-sd` crate themselves. Not appropriate for inclusion in the core slicing library. Skip this for the library; document as a recommended pattern for application developers instead.
