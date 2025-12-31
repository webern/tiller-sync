# MCP Service Description

[Tiller](https://tiller.com/) is a third-party service that aggregates and categorizes financial
transactions into a Google sheet.

This MCP server offers the user the ability to download these financial transactions into a local
SQLite database for further analysis and manipulation. The local datastore may also be written back
to the user's tiller Google sheet.

You, the AI agent, **MUST** read the full instructions by calling __initialize_service__ before
calling any other tools.
