import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

/** Enough markdown for the harness's own reports: headings, tables, lists, code, blockquotes, links. */
function inline(text: string, key: number): ReactNode {
  const parts: ReactNode[] = [];
  const re = /`([^`]+)`|\*\*([^*]+)\*\*|\[([^\]]+)\]\(([^)\s]+)\)/g;
  let last = 0, m: RegExpExecArray | null, i = 0;
  while ((m = re.exec(text))) {
    if (m.index > last) parts.push(text.slice(last, m.index));
    if (m[1] != null) parts.push(<code key={`${key}-${i++}`} className="rounded bg-panel-2 px-1 py-0.5 font-mono text-[12px]">{m[1]}</code>);
    else if (m[2] != null) parts.push(<strong key={`${key}-${i++}`}>{m[2]}</strong>);
    else if (m[3] != null) parts.push(/^https?:/.test(m[4]) ? <a key={`${key}-${i++}`} href={m[4]} target="_blank" rel="noopener" className="text-info underline underline-offset-2">{m[3]}</a> : <span key={`${key}-${i++}`}>{m[3]}</span>);
    last = re.lastIndex;
  }
  if (last < text.length) parts.push(text.slice(last));
  return <>{parts}</>;
}

export function Markdown({ text }: { text: string }) {
  const lines = text.replace(/\r/g, "").split("\n");
  const blocks: ReactNode[] = [];
  let i = 0, key = 0;

  while (i < lines.length) {
    const line = lines[i];
    if (/^```/.test(line)) {
      const body: string[] = [];
      for (i++; i < lines.length && !/^```/.test(lines[i]); i++) body.push(lines[i]);
      i++;
      blocks.push(<pre key={key++} className="my-2 overflow-auto rounded-md border border-border bg-panel-2 p-3 font-mono text-[12px]">{body.join("\n")}</pre>);
    } else if (/^\s*\|/.test(line)) {
      const rows: string[] = [];
      for (; i < lines.length && /^\s*\|/.test(lines[i]); i++) rows.push(lines[i]);
      const cells = (r: string) => r.trim().replace(/^\||\|$/g, "").split("|").map((c) => c.trim());
      const isSep = (r: string) => /^\s*\|?[\s:|-]+\|?\s*$/.test(r) && r.includes("-");
      const head = rows.length > 1 && isSep(rows[1]) ? cells(rows[0]) : null;
      const body = rows.slice(head ? 2 : 0);
      blocks.push(
        <div key={key++} className="my-2 overflow-x-auto">
          <table className="w-full border-collapse text-[13px]">
            {head && <thead><tr>{head.map((c, j) => <th key={j} className="border-b border-border bg-panel-2 px-2.5 py-1.5 text-left text-[12px] font-semibold text-muted">{inline(c, j)}</th>)}</tr></thead>}
            <tbody>{body.map((r, ri) => <tr key={ri}>{cells(r).map((c, ci) => <td key={ci} className="border-b border-border px-2.5 py-1.5 align-top">{inline(c, ci)}</td>)}</tr>)}</tbody>
          </table>
        </div>
      );
    } else if (/^#{1,6}\s/.test(line)) {
      const level = Math.min(line.match(/^#+/)![0].length, 3);
      const cls = level === 1 ? "mt-1 mb-3 text-[19px] font-semibold" : level === 2 ? "mt-5 mb-2 text-[16px] font-semibold" : "mt-4 mb-1.5 text-[14px] font-semibold";
      const content = inline(line.replace(/^#+\s*/, ""), key);
      blocks.push(level === 1 ? <h1 key={key++} className={cls}>{content}</h1> : level === 2 ? <h2 key={key++} className={cls}>{content}</h2> : <h3 key={key++} className={cls}>{content}</h3>);
      i++;
    } else if (/^\s*([-*]|\d+\.)\s/.test(line)) {
      const ordered = /^\s*\d+\./.test(line);
      const items: string[] = [];
      for (; i < lines.length && (/^\s*([-*]|\d+\.)\s/.test(lines[i]) || (/^\s{2,}\S/.test(lines[i]) && items.length)); i++) {
        if (/^\s*([-*]|\d+\.)\s/.test(lines[i])) items.push(lines[i].replace(/^\s*([-*]|\d+\.)\s/, ""));
        else items[items.length - 1] += " " + lines[i].trim();
      }
      const List = ordered ? "ol" : "ul";
      blocks.push(<List key={key++} className={cn("my-2 pl-5 text-[13px]", ordered ? "list-decimal" : "list-disc")}>{items.map((it, j) => <li key={j}>{inline(it, j)}</li>)}</List>);
    } else if (/^>\s?/.test(line)) {
      const body: string[] = [];
      for (; i < lines.length && /^>\s?/.test(lines[i]); i++) body.push(lines[i].replace(/^>\s?/, ""));
      blocks.push(<blockquote key={key++} className="my-2 border-l-2 border-border py-0.5 pl-3 text-muted">{inline(body.join(" "), key)}</blockquote>);
    } else if (/^\s*(---|\*\*\*)\s*$/.test(line)) {
      blocks.push(<hr key={key++} className="my-3 border-border" />);
      i++;
    } else if (!line.trim()) {
      i++;
    } else {
      const body: string[] = [];
      for (; i < lines.length && lines[i].trim() && !/^(```|#{1,6}\s|\s*\||\s*([-*]|\d+\.)\s|>)/.test(lines[i]); i++) body.push(lines[i]);
      blocks.push(<p key={key++} className="my-2 text-[13px] leading-relaxed">{inline(body.join(" "), key)}</p>);
    }
  }
  return <div className="max-w-none break-words">{blocks}</div>;
}
