import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-[11px] font-semibold leading-4 whitespace-nowrap",
  {
    variants: {
      variant: {
        neutral: "border-border bg-panel-2 text-muted",
        accent: "border-transparent bg-accent-soft text-accent",
        live: "border-transparent bg-live-soft text-live",
        success: "border-transparent bg-success-soft text-success",
        destructive: "border-transparent bg-destructive-soft text-destructive",
        warning: "border-transparent bg-warning-soft text-warning",
        info: "border-transparent bg-info-soft text-info",
      },
    },
    defaultVariants: { variant: "neutral" },
  }
);

export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement>, VariantProps<typeof badgeVariants> {}

export function Badge({ className, variant, ...props }: BadgeProps) {
  return <span className={cn(badgeVariants({ variant }), className)} {...props} />;
}
