<script lang="ts" module>
  let instances = 0;
</script>

<script lang="ts">
  // Offline icons (lucide and the generated file icons) in Iconify JSON form:
  // an SVG body drawn in a width × height box. The bodies come from build-time
  // data in this repository, never from files or the network.
  let {
    icon,
    width = '1em',
    height = '1em',
    class: className,
  }: {
    icon: { body: string; width?: number; height?: number };
    width?: number | string;
    height?: number | string;
    class?: string;
  } = $props();

  // Gradient, mask and clip-path IDs must be unique for every icon on the page.
  const suffix = `-icon${instances++}`;
  let body = $derived(
    icon.body.includes('id=')
      ? icon.body
          .replace(/\bid="([^"]+)"/g, `id="$1${suffix}"`)
          .replace(/url\(#([^)]+)\)/g, `url(#$1${suffix})`)
          .replace(/href="#([^"]+)"/g, `href="#$1${suffix}"`)
      : icon.body,
  );
</script>

<svg
  xmlns="http://www.w3.org/2000/svg"
  {width}
  {height}
  viewBox={`0 0 ${icon.width ?? 16} ${icon.height ?? 16}`}
  class={className}
  aria-hidden="true"
  role="img">{@html body}</svg
>
