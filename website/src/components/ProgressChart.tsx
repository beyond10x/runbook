import type {ReactNode} from 'react';

/**
 * Scopes the committed progress image's own stylesheet to its frame and moves its dark rules from
 * the reader's system setting to the site's theme switch, so the image follows the theme the page
 * shows. The image itself is `docs/img/progress.svg`, written by `sdc-experiment progress`.
 */
export function scopeProgressSvg(svg: string, scope: string): string {
  return svg
    .replace(/id="(title|desc)"/g, `id="${scope}-$1"`)
    .replace(/aria-labelledby="title desc"/, `aria-labelledby="${scope}-title ${scope}-desc"`)
    .replace(/<style>([\s\S]*?)<\/style>/, (_m, css: string) => {
      const media = /@media \(prefers-color-scheme:dark\)\{([\s\S]*)\}\s*$/;
      const dark = media.exec(css)?.[1] ?? '';
      const base = css.replace(media, '');
      const scoped = (rules: string, prefix: string) =>
        rules
          .split('}')
          .map((r) => r.trim())
          .filter(Boolean)
          .map((r) => {
            const [sel, decl] = r.split('{');
            return sel.split(',').map((s) => `${prefix} .${scope} ${s.trim()}`).join(',') + `{${decl}}`;
          })
          .join('');
      return `<style>${scoped(base, '')}${scoped(dark, "[data-theme='dark']")}</style>`;
    });
}

export default function ProgressChart({svg}: {svg: string}): ReactNode {
  return (
    <figure className="sdc-progress-figure">
      <div className="sdc-progress-viewport" tabIndex={0} role="region" aria-label="Progress chart; scroll sideways on a narrow screen">
        <div className="sdc-progress" dangerouslySetInnerHTML={{__html: scopeProgressSvg(svg, 'sdc-progress')}} />
      </div>
      <figcaption className="sdc-muted">
        <code>docs/img/progress.svg</code>, written by <code>sdc-experiment progress</code> from the experiment records.
        Each dot is one run; a filled dot is a clean pass.
      </figcaption>
    </figure>
  );
}
