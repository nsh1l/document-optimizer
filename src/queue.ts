import type { Job, Report } from "./types.ts";

export const supportedExtensions = ["xlsx", "xlsm", "docx", "pptx", "pdf"] as const;

export function filterSupportedPaths(paths: string[]) {
  const accepted: string[] = [];
  const unsupported: string[] = [];

  for (const path of paths) {
    const fileName = path.split(/[\\/]/).pop() ?? path;
    const extension = fileName.split(".").pop()?.toLowerCase() ?? "";
    (supportedExtensions.includes(extension as (typeof supportedExtensions)[number]) ? accepted : unsupported).push(path);
  }

  return { accepted, unsupported };
}

export function suggestedOutput(input: string, directory = ""): string {
  const name = input.split(/[\\/]/).pop() ?? input;
  const renamed = name.replace(/(\.[^.]+)$/, "_optimized$1");
  return directory ? `${directory.replace(/[\\/]$/, "")}/${renamed}` : input.slice(0, -name.length) + renamed;
}

export async function runQueue(jobs: Job[], execute: (job: Job) => Promise<Report>, update: (id: string, patch: Partial<Job>) => void, stopped: () => boolean) {
  for (const job of jobs.filter(job => job.status === "pending")) {
    if (stopped()) { update(job.id, { status: "cancelled" }); continue; }
    update(job.id, { status: "running", error: undefined });
    try {
      const report = await execute(job);
      update(job.id, { status: report.unchanged ? "unchanged" : "done", report });
    } catch (error) { update(job.id, { status: "error", error: String(error) }); }
  }
}
