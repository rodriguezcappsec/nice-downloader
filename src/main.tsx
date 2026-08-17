import { Component, StrictMode, type ErrorInfo, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles.css";

// The organizer is designed as a dark media room. A persisted theme preference can
// extend this later without flashing the light palette during desktop startup.
document.documentElement.classList.add("dark");

class AppErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Nice Downloader interface failed to render", error, info.componentStack);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return <main className="grid min-h-screen place-items-center bg-canvas p-8 text-ink"><section className="panel-shadow max-w-xl rounded-panel border border-danger/35 bg-panel p-6" role="alert"><p className="eyebrow">Interface recovery</p><h1 className="mt-3 text-2xl font-semibold">Nice Downloader could not draw this workspace.</h1><p className="mt-3 text-[13px] leading-5 text-muted">Your downloads and library are unchanged. Reload the interface, and open technical details if the problem continues.</p><details className="mt-4 rounded-control bg-elevated/55 p-3 text-[12px] text-muted"><summary className="cursor-pointer font-semibold text-ink">Technical details</summary><pre className="number-font mt-3 max-h-48 overflow-auto whitespace-pre-wrap">{this.state.error.message}</pre></details><button className="mt-5 h-10 rounded-control bg-accent px-4 text-[13px] font-semibold text-accentInk" onClick={() => window.location.reload()}>Reload interface</button></section></main>;
  }
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <AppErrorBoundary><App /></AppErrorBoundary>
  </StrictMode>,
);
