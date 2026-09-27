import * as React from "react";
import { cn } from "@/lib/utils";

export const Input = React.forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>(
  ({ className, ...props }, ref) => (
    <input
      ref={ref}
      className={cn(
        "flex h-9 w-full rounded-md border border-border bg-panel-2 px-3 text-sm text-foreground placeholder:text-muted outline-none transition-colors focus:border-accent disabled:opacity-40",
        className
      )}
      {...props}
    />
  )
);
Input.displayName = "Input";

export const Textarea = React.forwardRef<HTMLTextAreaElement, React.TextareaHTMLAttributes<HTMLTextAreaElement>>(
  ({ className, ...props }, ref) => (
    <textarea
      ref={ref}
      className={cn(
        "flex w-full rounded-md border border-border bg-panel-2 px-3 py-2 font-mono text-[12.5px] text-foreground placeholder:text-muted placeholder:font-sans outline-none transition-colors focus:border-accent disabled:opacity-40",
        className
      )}
      {...props}
    />
  )
);
Textarea.displayName = "Textarea";
