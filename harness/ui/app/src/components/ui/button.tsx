import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

// Achromatic by design: primary is solid foreground-on-black (max contrast, no colour needed to
// read as "the" action), everything else is a quiet border. Violet/cyan stay reserved for meaning
// (live status, agent identity) rather than button chrome — see D-101/the flat-theme decisions.
const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-semibold " +
    "transition-[background-color,border-color,color,transform] duration-150 active:scale-[0.97] " +
    "disabled:pointer-events-none disabled:opacity-40 disabled:active:scale-100 " +
    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-foreground/50 focus-visible:ring-offset-2 focus-visible:ring-offset-background",
  {
    variants: {
      variant: {
        primary: "bg-foreground text-background hover:bg-white",
        default: "border border-border bg-transparent text-foreground hover:bg-panel-2 hover:border-[#3a3a3a]",
        outline: "border border-border bg-transparent text-muted hover:text-foreground hover:border-foreground/40",
        danger: "border border-destructive/40 bg-transparent text-destructive hover:bg-destructive-soft",
        ghost: "text-muted hover:text-foreground hover:bg-panel-2",
      },
      size: {
        default: "h-9 px-4",
        sm: "h-8 px-3 text-xs",
        icon: "h-9 w-9 shrink-0 rounded-full",
      },
    },
    defaultVariants: { variant: "default", size: "default" },
  }
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {}

export function Button({ className, variant, size, ...props }: ButtonProps) {
  return <button className={cn(buttonVariants({ variant, size }), className)} {...props} />;
}
