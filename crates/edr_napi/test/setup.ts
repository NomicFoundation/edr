// Diagnostic for the recurring 6h CI hang: if the process is still alive a
// few seconds after the last test, dump active async resources and force-exit
// so CI surfaces *what* is keeping the event loop open (likely a leaked
// edr_napi handle / tokio runtime / HTTP socket).
after(function () {
  setTimeout(() => {
    const active =
      typeof (process as any).getActiveResourcesInfo === "function"
        ? (process as any).getActiveResourcesInfo()
        : [];
    // eslint-disable-next-line no-console
    console.error(
      `[mocha-exit-diagnostic] process still alive after suite; active resources (${active.length}): ${JSON.stringify(active)}`
    );
    process.exit(0);
  }, 5000).unref();
});
