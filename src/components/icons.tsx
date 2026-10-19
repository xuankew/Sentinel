// 统一的线性图标（stroke currentColor），保持三栏界面视觉一致。
type P = { size?: number; class?: string };

function Svg({ size = 18, class: cls, children }: P & { children: any }) {
  return (
    <svg
      class={cls}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.8"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      {children}
    </svg>
  );
}

export const ShieldIcon = (p: P) => (
  <Svg {...p}>
    <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
    <path d="M9 12l2 2 4-4" />
  </Svg>
);
export const KeyIcon = (p: P) => (
  <Svg {...p}>
    <circle cx="7.5" cy="15.5" r="4.5" />
    <path d="M10.8 12.2L20 3M17 6l2.5 2.5M14.5 8.5L17 11" />
  </Svg>
);
export const StarIcon = ({ filled, ...p }: P & { filled?: boolean }) => (
  <Svg {...p}>
    <path
      d="M12 3.5l2.6 5.4 5.9.8-4.3 4.1 1.1 5.9L12 16.9 6.7 19.7l1.1-5.9L3.5 9.7l5.9-.8z"
      fill={filled ? "currentColor" : "none"}
    />
  </Svg>
);
export const GridIcon = (p: P) => (
  <Svg {...p}>
    <rect x="3" y="3" width="7" height="7" rx="1.5" />
    <rect x="14" y="3" width="7" height="7" rx="1.5" />
    <rect x="3" y="14" width="7" height="7" rx="1.5" />
    <rect x="14" y="14" width="7" height="7" rx="1.5" />
  </Svg>
);
export const SearchIcon = (p: P) => (
  <Svg {...p}>
    <circle cx="11" cy="11" r="7" />
    <path d="M20 20l-3.5-3.5" />
  </Svg>
);
export const PlusIcon = (p: P) => (
  <Svg {...p}>
    <path d="M12 5v14M5 12h14" />
  </Svg>
);
export const CopyIcon = (p: P) => (
  <Svg {...p}>
    <rect x="9" y="9" width="12" height="12" rx="2" />
    <path d="M5 15V5a2 2 0 012-2h8" />
  </Svg>
);
export const CheckIcon = (p: P) => (
  <Svg {...p}>
    <path d="M4 12.5l5 5L20 6.5" />
  </Svg>
);
export const EyeIcon = (p: P) => (
  <Svg {...p}>
    <path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7-10-7-10-7z" />
    <circle cx="12" cy="12" r="3" />
  </Svg>
);
export const EyeOffIcon = (p: P) => (
  <Svg {...p}>
    <path d="M3 3l18 18M10.6 6.2A9.9 9.9 0 0112 6c6.4 0 10 6 10 6a17 17 0 01-3.3 3.9M6.3 8.2A16.7 16.7 0 002 12s3.6 6 10 6a9.6 9.6 0 003.4-.6" />
    <path d="M9.9 9.9a3 3 0 004.2 4.2" />
  </Svg>
);
export const EditIcon = (p: P) => (
  <Svg {...p}>
    <path d="M4 20h4L20 8l-4-4L4 16z" />
    <path d="M14.5 5.5L18.5 9.5" />
  </Svg>
);
export const TrashIcon = (p: P) => (
  <Svg {...p}>
    <path d="M4 7h16M10 7V5h4v2M6 7l1 13h10l1-13" />
  </Svg>
);
export const LockIcon = (p: P) => (
  <Svg {...p}>
    <rect x="4.5" y="10" width="15" height="11" rx="2" />
    <path d="M8 10V7a4 4 0 018 0v3" />
  </Svg>
);
export const LinkIcon = (p: P) => (
  <Svg {...p}>
    <path d="M10 13a5 5 0 007 0l2-2a5 5 0 00-7-7l-1 1" />
    <path d="M14 11a5 5 0 00-7 0l-2 2a5 5 0 007 7l1-1" />
  </Svg>
);
export const XIcon = (p: P) => (
  <Svg {...p}>
    <path d="M6 6l12 12M18 6L6 18" />
  </Svg>
);
export const RefreshIcon = (p: P) => (
  <Svg {...p}>
    <path d="M20 11A8 8 0 006.3 6.3L4 8.5" />
    <path d="M4 5v3.5h3.5" />
    <path d="M4 13a8 8 0 0013.7 4.7L20 15.5" />
    <path d="M20 19v-3.5h-3.5" />
  </Svg>
);
export const TagIcon = (p: P) => (
  <Svg {...p}>
    <path d="M3 12V4h8l9 9-8 8z" />
    <circle cx="7.5" cy="7.5" r="1.3" />
  </Svg>
);
export const NoteIcon = (p: P) => (
  <Svg {...p}>
    <path d="M6 3h9l4 4v14H6z" />
    <path d="M15 3v4h4M9 12h7M9 16h7" />
  </Svg>
);
export const SyncIcon = (p: P) => (
  <Svg {...p}>
    <path d="M4 12a8 8 0 0113.7-5.6L20 8.5" />
    <path d="M20 4v4.5h-4.5" />
    <path d="M20 12a8 8 0 01-13.7 5.6L4 15.5" />
    <path d="M4 20v-4.5h4.5" />
  </Svg>
);
export const GearIcon = (p: P) => (
  <Svg {...p}>
    <circle cx="12" cy="12" r="3.2" />
    <path d="M12 2.5l1.4 2.6 2.9-.4.6 2.9 2.6 1.4-1.6 2.4 1.6 2.4-2.6 1.4-.6 2.9-2.9-.4L12 21.5l-1.4-2.6-2.9.4-.6-2.9L4.5 15l1.6-2.4L4.5 10l2.6-1.4.6-2.9 2.9.4z" />
  </Svg>
);
export const CloudIcon = (p: P) => (
  <Svg {...p}>
    <path d="M7 18a4 4 0 01-.5-8A5.5 5.5 0 0117 9.5a3.8 3.8 0 01.5 8.5z" />
  </Svg>
);
export const WarnIcon = (p: P) => (
  <Svg {...p}>
    <path d="M12 3.5L22 20H2z" />
    <path d="M12 9.5v5M12 17.2v.3" />
  </Svg>
);
export const DownloadIcon = (p: P) => (
  <Svg {...p}>
    <path d="M12 3v12M7 11l5 5 5-5" />
    <path d="M4 20h16" />
  </Svg>
);
export const UploadIcon = (p: P) => (
  <Svg {...p}>
    <path d="M12 16V4M7 9l5-5 5 5" />
    <path d="M4 20h16" />
  </Svg>
);
export const FingerprintIcon = (p: P) => (
  <Svg {...p}>
    <path d="M12 4a8 8 0 00-8 8v1M20 12a8 8 0 00-3-6.2" />
    <path d="M8 12a4 4 0 018 0v2a12 12 0 01-1.5 5.5" />
    <path d="M12 12v3a10 10 0 01-1 4.5M6.5 19A14 14 0 008 13" />
  </Svg>
);
export const PaletteIcon = (p: P) => (
  <Svg {...p}>
    <path d="M12 3a9 9 0 100 18c1.1 0 1.8-.9 1.8-1.8 0-.5-.2-.9-.5-1.2-.3-.3-.5-.7-.5-1.2 0-.9.8-1.8 1.8-1.8H16a5 5 0 005-5c0-3.9-4-7-9-7z" />
    <circle cx="7.5" cy="11.5" r="1" fill="currentColor" />
    <circle cx="10" cy="7.5" r="1" fill="currentColor" />
    <circle cx="14.5" cy="7.5" r="1" fill="currentColor" />
  </Svg>
);
export const QrIcon = (p: P) => (
  <Svg {...p}>
    <path d="M4 8V5.5A1.5 1.5 0 015.5 4H8M16 4h2.5A1.5 1.5 0 0120 5.5V8M20 16v2.5a1.5 1.5 0 01-1.5 1.5H16M8 20H5.5A1.5 1.5 0 014 18.5V16" />
    <rect x="7.5" y="7.5" width="3" height="3" rx="0.5" />
    <rect x="13.5" y="7.5" width="3" height="3" rx="0.5" />
    <rect x="7.5" y="13.5" width="3" height="3" rx="0.5" />
    <path d="M13.5 13.5h1.5v1.5h-1.5zM16.5 16.5H18v-1.5" />
  </Svg>
);
