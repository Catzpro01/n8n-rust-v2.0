// SPDX-License-Identifier: AGPL-3.0-or-later

import { render } from "preact";
import { useEffect, useState } from "preact/hooks";
import "./styles.css";

type Release = {
  product: string;
  version: string;
  build_commit: string;
};

type Readiness = {
  status: string;
  checks: { sqlite: { journal_mode: string; synchronous: string } };
};

type Resources = {
  cpu: { available: boolean; values: Record<string, number | null> };
  memory: { available: boolean; values: Record<string, number | null> };
};

type Snapshot = {
  release?: Release;
  readiness?: Readiness;
  resources?: Resources;
  error?: string;
};

function Mark() {
  return (
    <svg class="mark" viewBox="0 0 48 48" role="img" aria-label="Canopy Workbench placeholder mark">
      <path d="M8 29 24 7l16 22-16 12Z" fill="none" stroke="currentColor" stroke-width="3" />
      <circle cx="24" cy="25" r="5" fill="currentColor" />
    </svg>
  );
}

function App() {
  const [snapshot, setSnapshot] = useState<Snapshot>({});
  useEffect(() => {
    let active = true;
    Promise.all([
      fetch("/api/v1/release").then((response) => response.json() as Promise<Release>),
      fetch("/health/ready").then((response) => response.json() as Promise<Readiness>),
      fetch("/api/v1/resources").then((response) => response.json() as Promise<Resources>),
    ])
      .then(([release, readiness, resources]) => {
        if (active) setSnapshot({ release, readiness, resources });
      })
      .catch((error: unknown) => {
        if (active) setSnapshot({ error: error instanceof Error ? error.message : "Connection failed" });
      });
    return () => {
      active = false;
    };
  }, []);

  return (
    <main class="shell">
      <header class="masthead">
        <div class="identity">
          <Mark />
          <div>
            <p class="eyebrow">Independent automation workspace</p>
            <h1>Canopy Workbench</h1>
          </div>
        </div>
        <span class={`health ${snapshot.readiness?.status === "ready" ? "ready" : "waiting"}`}>
          <span aria-hidden="true" />
          {snapshot.readiness?.status ?? "connecting"}
        </span>
      </header>

      <section class="welcome" aria-labelledby="welcome-title">
        <p class="eyebrow">A calm place for dependable work</p>
        <h2 id="welcome-title">Shape the work. See the proof.</h2>
        <p>This first shell is connected to the local daemon. Workflow authoring arrives in the next slices.</p>
        <button type="button" disabled title="Available after Owner setup">Create a workspace</button>
      </section>

      <section class="cards" aria-label="Installation identity">
        <article>
          <span class="number">01</span>
          <h3>Release</h3>
          <strong>{snapshot.release ? `v${snapshot.release.version}` : "—"}</strong>
          <p>{snapshot.release?.build_commit ?? "Reading build identity…"}</p>
        </article>
        <article>
          <span class="number">02</span>
          <h3>Storage</h3>
          <strong>{snapshot.readiness?.checks.sqlite.journal_mode?.toUpperCase() ?? "—"}</strong>
          <p>{snapshot.readiness ? `${snapshot.readiness.checks.sqlite.synchronous.toUpperCase()} durability` : "Checking SQLite…"}</p>
        </article>
        <article>
          <span class="number">03</span>
          <h3>Resource view</h3>
          <strong>{snapshot.resources?.cpu.available ? "Observed" : "Unavailable"}</strong>
          <p>{snapshot.resources?.memory.available ? "Memory controller visible" : "Reported honestly"}</p>
        </article>
      </section>

      {snapshot.error && <p role="alert" class="error">Daemon connection failed: {snapshot.error}</p>}
      <footer>Placeholder identity · independently authored · no third-party editor assets</footer>
    </main>
  );
}

render(<App />, document.getElementById("app")!);
