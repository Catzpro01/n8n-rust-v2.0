---
status: accepted
---
# Own a durable provider-independent URL frontier

Each Crawl Plan owns canonicalization, deduplication, scope, depth, domain and session budgets, checkpoint and resume behavior, and parent-child relationships in a durable URL Frontier independent of any Scrape Adapter. Adapter fallback or migration therefore neither loses crawl state nor repeats completed pages without an explicit refresh policy.
