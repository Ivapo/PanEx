import type { LayoutNode, SplitDirection } from "./types.ts";

export function countLeaves(node: LayoutNode): number {
  if (node.type === "leaf") return 1;
  return countLeaves(node.first) + countLeaves(node.second);
}

export function collectLeafIds(node: LayoutNode): string[] {
  if (node.type === "leaf") return [node.paneId];
  return [...collectLeafIds(node.first), ...collectLeafIds(node.second)];
}

export function splitPane(
  root: LayoutNode,
  targetPaneId: string,
  newPaneId: string,
  direction: SplitDirection
): LayoutNode {
  if (root.type === "leaf") {
    if (root.paneId === targetPaneId) {
      return {
        type: "split",
        direction,
        first: { type: "leaf", paneId: targetPaneId },
        second: { type: "leaf", paneId: newPaneId },
        ratio: 0.5,
      };
    }
    return root;
  }

  return {
    ...root,
    first: splitPane(root.first, targetPaneId, newPaneId, direction),
    second: splitPane(root.second, targetPaneId, newPaneId, direction),
  };
}

export function removePane(
  root: LayoutNode,
  paneId: string
): LayoutNode | null {
  if (root.type === "leaf") {
    return root.paneId === paneId ? null : root;
  }

  const firstResult = removePane(root.first, paneId);
  const secondResult = removePane(root.second, paneId);

  if (firstResult === null) return secondResult;
  if (secondResult === null) return firstResult;

  return { ...root, first: firstResult, second: secondResult };
}
