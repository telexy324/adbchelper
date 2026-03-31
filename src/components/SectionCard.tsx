import { PropsWithChildren } from "react";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./ui/card";

interface SectionCardProps extends PropsWithChildren {
  eyebrow: string;
  title: string;
  description: string;
}

export function SectionCard({
  eyebrow,
  title,
  description,
  children,
}: SectionCardProps) {
  return (
    <Card className="border-border/70 bg-card/90 shadow-[0_20px_80px_-38px_rgba(15,23,42,0.35)]">
      <CardHeader className="space-y-3">
        <p className="text-xs font-medium uppercase tracking-[0.24em] text-muted-foreground">{eyebrow}</p>
        <CardTitle className="font-serif text-2xl">{title}</CardTitle>
        <CardDescription className="max-w-2xl text-sm leading-6">{description}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">{children}</CardContent>
    </Card>
  );
}
