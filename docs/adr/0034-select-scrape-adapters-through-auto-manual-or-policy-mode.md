---
status: accepted
---
# Select scrape adapters through auto, manual, or policy mode

Scrape Orchestrator supports automatic selection, manual pinning, and reusable policies. Automatic mode escalates from the cheapest capable compliant adapter—native Rust HTTP and parsing, Firecrawl or Scrapy, Crawlee and Playwright, then Camoufox or commercial remote providers—only from recorded evidence rather than racing every provider.
