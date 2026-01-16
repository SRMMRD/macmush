/**
 * ANSI escape code parser for terminal colors and text attributes
 *
 * Supports:
 * - Basic 16 colors (30-37, 90-97 for foreground; 40-47, 100-107 for background)
 * - 256-color palette (ESC[38;5;n for fg, ESC[48;5;n for bg)
 * - RGB/Truecolor (ESC[38;2;r;g;b for fg, ESC[48;2;r;g;b for bg)
 * - Text attributes (bold, dim, italic, underline, blink, reverse, hidden, strikethrough)
 */

// ANSI escape sequence regex
const ANSI_REGEX = /\x1b\[([0-9;]+)m/g;

// Basic 16 ANSI colors (matching xterm palette)
const ANSI_COLORS = {
  // Normal intensity
  30: '#000000', // black
  31: '#cd0000', // red
  32: '#00cd00', // green
  33: '#cdcd00', // yellow
  34: '#0000ee', // blue
  35: '#cd00cd', // magenta
  36: '#00cdcd', // cyan
  37: '#e5e5e5', // white

  // Bright/bold intensity
  90: '#7f7f7f', // bright black (gray)
  91: '#ff0000', // bright red
  92: '#00ff00', // bright green
  93: '#ffff00', // bright yellow
  94: '#5c5cff', // bright blue
  95: '#ff00ff', // bright magenta
  96: '#00ffff', // bright cyan
  97: '#ffffff', // bright white
};

// Background colors (same palette, different codes)
const ANSI_BG_COLORS = {
  40: ANSI_COLORS[30],
  41: ANSI_COLORS[31],
  42: ANSI_COLORS[32],
  43: ANSI_COLORS[33],
  44: ANSI_COLORS[34],
  45: ANSI_COLORS[35],
  46: ANSI_COLORS[36],
  47: ANSI_COLORS[37],

  100: ANSI_COLORS[90],
  101: ANSI_COLORS[91],
  102: ANSI_COLORS[92],
  103: ANSI_COLORS[93],
  104: ANSI_COLORS[94],
  105: ANSI_COLORS[95],
  106: ANSI_COLORS[96],
  107: ANSI_COLORS[97],
};

/**
 * Convert 256-color palette index to RGB hex
 */
function color256ToHex(index) {
  // Colors 0-15: standard colors (same as basic 16)
  if (index < 16) {
    const mapping = [30, 31, 32, 33, 34, 35, 36, 37, 90, 91, 92, 93, 94, 95, 96, 97];
    return ANSI_COLORS[mapping[index]];
  }

  // Colors 16-231: 6x6x6 color cube
  if (index >= 16 && index <= 231) {
    const i = index - 16;
    const r = Math.floor(i / 36);
    const g = Math.floor((i % 36) / 6);
    const b = i % 6;

    const toHex = (v) => {
      const val = v === 0 ? 0 : 55 + v * 40;
      return val.toString(16).padStart(2, '0');
    };

    return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
  }

  // Colors 232-255: grayscale ramp
  if (index >= 232 && index <= 255) {
    const gray = 8 + (index - 232) * 10;
    const hex = gray.toString(16).padStart(2, '0');
    return `#${hex}${hex}${hex}`;
  }

  return '#e5e5e5'; // default
}

/**
 * Parse ANSI escape sequences and return styled text segments
 *
 * @param {string} text - Text with ANSI codes
 * @returns {Array} Array of {text, style} objects
 */
export function parseAnsi(text) {
  if (!text) return [{ text: '', style: {} }];

  const segments = [];
  let currentStyle = {
    color: null,
    backgroundColor: null,
    bold: false,
    dim: false,
    italic: false,
    underline: false,
    blink: false,
    reverse: false,
    hidden: false,
    strikethrough: false,
  };

  let lastIndex = 0;
  let match;

  // Reset regex state
  ANSI_REGEX.lastIndex = 0;

  while ((match = ANSI_REGEX.exec(text)) !== null) {
    // Add text before this escape sequence
    if (match.index > lastIndex) {
      const textSegment = text.substring(lastIndex, match.index);
      if (textSegment) {
        segments.push({
          text: textSegment,
          style: { ...currentStyle }
        });
      }
    }

    // Parse the SGR (Select Graphic Rendition) codes
    const codes = match[1].split(';').map(Number);
    let i = 0;

    while (i < codes.length) {
      const code = codes[i];

      // Reset all attributes
      if (code === 0) {
        currentStyle = {
          color: null,
          backgroundColor: null,
          bold: false,
          dim: false,
          italic: false,
          underline: false,
          blink: false,
          reverse: false,
          hidden: false,
          strikethrough: false,
        };
      }
      // Bold
      else if (code === 1) {
        currentStyle.bold = true;
      }
      // Dim
      else if (code === 2) {
        currentStyle.dim = true;
      }
      // Italic
      else if (code === 3) {
        currentStyle.italic = true;
      }
      // Underline
      else if (code === 4) {
        currentStyle.underline = true;
      }
      // Blink
      else if (code === 5) {
        currentStyle.blink = true;
      }
      // Reverse
      else if (code === 7) {
        currentStyle.reverse = true;
      }
      // Hidden
      else if (code === 8) {
        currentStyle.hidden = true;
      }
      // Strikethrough
      else if (code === 9) {
        currentStyle.strikethrough = true;
      }
      // Normal intensity (not bold or dim)
      else if (code === 22) {
        currentStyle.bold = false;
        currentStyle.dim = false;
      }
      // Not italic
      else if (code === 23) {
        currentStyle.italic = false;
      }
      // Not underline
      else if (code === 24) {
        currentStyle.underline = false;
      }
      // Not blink
      else if (code === 25) {
        currentStyle.blink = false;
      }
      // Not reverse
      else if (code === 27) {
        currentStyle.reverse = false;
      }
      // Not hidden
      else if (code === 28) {
        currentStyle.hidden = false;
      }
      // Not strikethrough
      else if (code === 29) {
        currentStyle.strikethrough = false;
      }
      // Foreground colors (30-37, 90-97)
      else if (ANSI_COLORS[code]) {
        currentStyle.color = ANSI_COLORS[code];
      }
      // 256-color foreground
      else if (code === 38 && codes[i + 1] === 5) {
        currentStyle.color = color256ToHex(codes[i + 2]);
        i += 2;
      }
      // RGB foreground
      else if (code === 38 && codes[i + 1] === 2) {
        const r = codes[i + 2];
        const g = codes[i + 3];
        const b = codes[i + 4];
        currentStyle.color = `#${r.toString(16).padStart(2, '0')}${g.toString(16).padStart(2, '0')}${b.toString(16).padStart(2, '0')}`;
        i += 4;
      }
      // Default foreground
      else if (code === 39) {
        currentStyle.color = null;
      }
      // Background colors (40-47, 100-107)
      else if (ANSI_BG_COLORS[code]) {
        currentStyle.backgroundColor = ANSI_BG_COLORS[code];
      }
      // 256-color background
      else if (code === 48 && codes[i + 1] === 5) {
        currentStyle.backgroundColor = color256ToHex(codes[i + 2]);
        i += 2;
      }
      // RGB background
      else if (code === 48 && codes[i + 1] === 2) {
        const r = codes[i + 2];
        const g = codes[i + 3];
        const b = codes[i + 4];
        currentStyle.backgroundColor = `#${r.toString(16).padStart(2, '0')}${g.toString(16).padStart(2, '0')}${b.toString(16).padStart(2, '0')}`;
        i += 4;
      }
      // Default background
      else if (code === 49) {
        currentStyle.backgroundColor = null;
      }

      i++;
    }

    lastIndex = match.index + match[0].length;
  }

  // Add remaining text after last escape sequence
  if (lastIndex < text.length) {
    segments.push({
      text: text.substring(lastIndex),
      style: { ...currentStyle }
    });
  }

  // If no segments were created, return the original text with no style
  if (segments.length === 0) {
    segments.push({ text, style: {} });
  }

  return segments;
}

/**
 * Convert style object to CSS properties
 */
export function styleToCSS(style) {
  const css = {};

  if (style.color) {
    css.color = style.color;
  }

  if (style.backgroundColor) {
    css.backgroundColor = style.backgroundColor;
  }

  if (style.bold) {
    css.fontWeight = 'bold';
  }

  if (style.dim) {
    css.opacity = '0.6';
  }

  if (style.italic) {
    css.fontStyle = 'italic';
  }

  if (style.underline) {
    css.textDecoration = style.strikethrough ? 'underline line-through' : 'underline';
  } else if (style.strikethrough) {
    css.textDecoration = 'line-through';
  }

  if (style.blink) {
    css.animation = 'blink 1s step-start infinite';
  }

  if (style.reverse) {
    // Swap foreground and background
    if (style.color || style.backgroundColor) {
      css.color = style.backgroundColor || '#1a1a1a';
      css.backgroundColor = style.color || '#e5e5e5';
    } else {
      css.color = '#1a1a1a';
      css.backgroundColor = '#e5e5e5';
    }
  }

  if (style.hidden) {
    css.opacity = '0';
  }

  return css;
}
