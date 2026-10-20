export function escapeHtml(input: string): string {
  return input
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function inline(text: string): string {
  let output = escapeHtml(text);
  output = output.replace(/`([^`\n]+)`/g, '<code>$1</code>');
  output = output.replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>');
  output = output.replace(/(^|[^*])\*([^*\n]+)\*/g, '$1<em>$2</em>');
  output = output.replace(
    /\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g,
    '<a href="$2" target="_blank" rel="noreferrer noopener">$1</a>',
  );
  return output;
}

function renderText(source: string): string {
  if (!source.trim()) return '';
  const blocks = source.trim().split(/\n{2,}/);
  return blocks
    .map((block) => {
      const heading = block.match(/^(#{1,4})\s+(.*)$/);
      if (heading) {
        const level = Math.min(heading[1].length + 1, 5);
        return `<h${level}>${inline(heading[2])}</h${level}>`;
      }

      if (/^>\s?/m.test(block)) {
        const quote = block
          .split('\n')
          .map((line) => line.replace(/^>\s?/, ''))
          .join(' ');
        return `<blockquote>${inline(quote)}</blockquote>`;
      }

      const lines = block.split('\n');
      if (lines.every((line) => /^\s*[-*]\s+/.test(line))) {
        const items = lines
          .map((line) => `<li>${inline(line.replace(/^\s*[-*]\s+/, ''))}</li>`)
          .join('');
        return `<ul>${items}</ul>`;
      }
      if (lines.every((line) => /^\s*\d+\.\s+/.test(line))) {
        const items = lines
          .map((line) => `<li>${inline(line.replace(/^\s*\d+\.\s+/, ''))}</li>`)
          .join('');
        return `<ol>${items}</ol>`;
      }

      return `<p>${inline(block).replaceAll('\n', '<br />')}</p>`;
    })
    .join('');
}

export function renderMarkdown(source: string): string {
  const normalized = (source ?? '').replaceAll('\r\n', '\n');
  const segments = normalized.split('```');
  let html = '';

  for (let index = 0; index < segments.length; index += 1) {
    const segment = segments[index];
    if (index % 2 === 1) {
      const lines = segment.split('\n');
      const language = (lines.shift() ?? '').trim();
      const code = lines.join('\n').replace(/\n$/, '');
      html += `<div class="code-block"><div class="code-block-head"><span>${escapeHtml(
        language || 'code',
      )}</span><button class="copy-code" type="button">Copy</button></div><pre><code>${escapeHtml(
        code,
      )}</code></pre></div>`;
    } else {
      html += renderText(segment);
    }
  }

  return html;
}
