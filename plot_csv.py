#!/usr/bin/env python3
import argparse
import csv
import sys


def read_events(csv_path):
    def normalize(label: str) -> str:
        s = label.strip()
        # Nested: Actor(Variant { ... }) -> Actor.Variant
        if "(" in s and ")" in s:
            actor = s.split("(", 1)[0].strip()
            inner = s.split("(", 1)[1].rsplit(")", 1)[0]
            variant = inner.split("{", 1)[0].split("(", 1)[0].strip()
            if actor and variant:
                return f"{actor}.{variant}"
        # Fallback: ActorVariant -> Actor.Variant
        head = s.split("{", 1)[0].split("(", 1)[0].strip()
        for actor in ("Farm", "Mill", "Bakery"):
            if head.startswith(actor) and len(head) > len(actor):
                return f"{actor}.{head[len(actor):]}"
        return head

    times_by_label = {}
    with open(csv_path, newline="") as f:
        r = csv.DictReader(f)
        required = {"months", "event"}
        if not required.issubset(r.fieldnames or {}):
            raise SystemExit("Events CSV missing required headers months,event")
        for row in r:
            try:
                t = float(row["months"])
                label = row.get("event", "")
                extras = row.get(None)
                if extras:
                    label = ",".join([label] + list(extras))
                label = label.strip()
            except Exception:
                continue
            key = normalize(label)
            times_by_label.setdefault(key, []).append(t)
    return times_by_label


def main():
    ap = argparse.ArgumentParser(description="Plot Stronghold DES CSV (months,wheat,flour,bread) with optional event overlay")
    ap.add_argument("csv", help="Input history CSV (months,wheat,flour,bread)")
    ap.add_argument("out", nargs="?", default=None, help="Optional output PNG path; if omitted and --show is set, just shows window")
    ap.add_argument("--show", action="store_true", help="Show an interactive window")
    ap.add_argument("--events", default=None, help="Optional events CSV (months,event) to overlay")
    ap.add_argument("--title", default="Stronghold Bread Pipeline", help="Plot title")
    args = ap.parse_args()

    months, wheat, flour, bread = [], [], [], []
    with open(args.csv, newline="") as f:
        r = csv.DictReader(f)
        required = {"months", "wheat", "flour", "bread"}
        if not required.issubset(r.fieldnames or {}):
            print("CSV missing required headers months,wheat,flour,bread", file=sys.stderr)
            sys.exit(2)
        for row in r:
            try:
                t = float(row["months"]) ; w = int(row["wheat"]) ; fl = int(row["flour"]) ; br = int(row["bread"]) 
            except Exception:
                continue
            months.append(t) ; wheat.append(w) ; flour.append(fl) ; bread.append(br)

    try:
        import matplotlib.pyplot as plt
    except Exception as e:
        print("matplotlib not available:", e, file=sys.stderr)
        sys.exit(3)

    import matplotlib.pyplot as plt
    fig, ax = plt.subplots(figsize=(9, 5))
    lines = []
    if months:
        l1, = ax.step(months, wheat, where="post", label="Wheat", linewidth=1.8)
        l2, = ax.step(months, flour, where="post", label="Flour", linewidth=1.8)
        l3, = ax.step(months, bread, where="post", label="Bread", linewidth=1.8)
        for ln in (l1, l2, l3):
            try:
                ln.set_pickradius(5)
            except Exception:
                pass
            lines.append(ln)
    # Optional events overlay
    hover_events = []
    if args.events:
        try:
            events_by_label = read_events(args.events)
        except SystemExit:
            raise
        except Exception as e:
            print(f"Failed to read events CSV: {e}", file=sys.stderr)
            events_by_label = {}
        palette = ["tab:blue", "tab:orange", "tab:green", "tab:red", "tab:purple", "tab:brown", "tab:pink", "gold"]
        for i, (label, times) in enumerate(sorted(events_by_label.items())):
            color = palette[i % len(palette)]
            for t in sorted(times):
                ln = ax.axvline(x=t, color=color, alpha=0.35, linewidth=1.0)
                try:
                    ln.set_pickradius(5)
                except Exception:
                    pass
                hover_events.append((ln, label, t))
            # legend stub for event type
            ax.plot([], [], color=color, alpha=0.6, label=f"{label} ({len(times)})")
    ax.set_xlabel("Time (months)")
    ax.set_ylabel("Amount in storage")
    ax.set_title(args.title)
    ax.grid(True, alpha=0.3)
    ax.legend()
    fig.tight_layout()

    # Hover tooltip: show nearest line name and value
    annot = ax.annotate(
        "",
        xy=(0, 0),
        xytext=(10, 10),
        textcoords="offset points",
        bbox=dict(boxstyle="round", fc="w", ec="0.5", alpha=0.9),
        arrowprops=dict(arrowstyle="->", alpha=0.5),
    )
    annot.set_visible(False)

    def update_annot(ln, x, y):
        annot.xy = (x, y)
        annot.set_text(f"{ln.get_label()}: {y}\n@ t={x:.3f}")
        annot.set_visible(True)

    def on_move(event):
        if event.inaxes != ax:
            if annot.get_visible():
                annot.set_visible(False)
                fig.canvas.draw_idle()
            return
        # Priority: data series first
        for ln in lines:
            try:
                hit, info = ln.contains(event)
            except Exception:
                hit, info = False, {}
            if hit:
                xd = ln.get_xdata(orig=False)
                yd = ln.get_ydata(orig=False)
                ind = None
                if isinstance(info, dict):
                    inds = info.get("ind", [])
                    if len(inds):
                        ind = inds[0]
                if ind is None:
                    try:
                        import bisect
                        ind = max(0, min(len(xd) - 1, bisect.bisect_left(xd, event.xdata)))
                    except Exception:
                        ind = 0
                x = float(xd[ind])
                y = yd[ind]
                update_annot(ln, x, y)
                fig.canvas.draw_idle()
                return
        # Then check event markers
        for ln, label, t in hover_events:
            try:
                hit, _ = ln.contains(event)
            except Exception:
                hit = False
            if hit:
                annot.xy = (event.xdata, event.ydata)
                annot.set_text(f"{label}\n@ t={t:.3f}")
                annot.set_visible(True)
                fig.canvas.draw_idle()
                return
        if annot.get_visible():
            annot.set_visible(False)
            fig.canvas.draw_idle()

    fig.canvas.mpl_connect("motion_notify_event", on_move)
    if args.out:
        fig.savefig(args.out, dpi=150)
        print(f"Saved plot to {args.out}")
    if args.show:
        plt.show()


if __name__ == "__main__":
    main()
