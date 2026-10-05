import MarkdownIt from "markdown-it";
import taskLists from "markdown-it-task-lists";
import DOMPurify from "dompurify";

const parser = new MarkdownIt({
  html: false,
  breaks: true,
  linkify: false,
  typographer: false,
}).use(taskLists, { enabled: true, label: true });

/** Render untrusted local Markdown into a safe HTML fragment. */
export function renderMarkdown(source: string): string {
  const rendered = parser.render(source);
  const clean = DOMPurify.sanitize(rendered, {
    USE_PROFILES: { html: true },
    FORBID_TAGS: ["img", "audio", "video", "iframe", "object", "embed", "style"],
    FORBID_ATTR: ["srcset", "autoplay", "onerror", "onclick", "onload"],
    ALLOW_UNKNOWN_PROTOCOLS: false,
  });

  const wrapper = document.createElement("template");
  wrapper.innerHTML = clean;
  // Images and raw embedded content are deliberately text-only in v1.
  wrapper.content.querySelectorAll("img, audio, video, iframe, object, embed").forEach((node) => {
    const label = node.getAttribute("alt") || "[embedded content omitted]";
    node.replaceWith(document.createTextNode(label));
  });
  wrapper.content.querySelectorAll("a").forEach((link) => {
    const href = link.getAttribute("href") || "";
    if (/^https?:\/\//i.test(href)) {
      link.setAttribute("target", "_blank");
      link.setAttribute("rel", "noopener noreferrer");
    } else if (href.startsWith("#")) {
      // Document anchors are safe and remain inside the webview.
      link.removeAttribute("target");
    } else {
      link.replaceWith(document.createTextNode(link.textContent || href));
    }
  });
  wrapper.content.querySelectorAll('input[type="checkbox"]').forEach((checkbox) => {
    checkbox.setAttribute("disabled", "");
    checkbox.setAttribute("aria-readonly", "true");
  });
  return wrapper.innerHTML;
}
