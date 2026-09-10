---
status: accepted
---
# Return typed scrape results with provenance

Each fetched resource yields a Scrape Result Bundle containing requested raw Artifacts, normalized documents, JSON-Schema records, links, optional screenshot or DOM evidence, and provenance including final URL, time, adapter, region, content hash, status, and extraction confidence. Identical content is content-addressed and referenced rather than stored repeatedly.
