# Security Policy

## Supported versions

glowfetch is in active development. Security fixes target the latest commit on the main branch.

## Reporting a vulnerability

Please report suspected vulnerabilities privately by email to n82238895@gmail.com. Include a description, the affected version, and reproduction steps. You can expect an initial response within a reasonable time, and coordinated disclosure once a fix is available.

## Scope

glowfetch reads local system information through sysinfo and WMI and renders it to the terminal. It does not open network listeners and does not transmit data. Reports that involve those local data paths are in scope.
