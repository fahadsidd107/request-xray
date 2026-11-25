import type { SVGProps } from 'react';

const paths = {
  logo: 'M2 17h4l3-10 4 14 3-9h6',
  traces: 'M4 6h12M8 12h12M4 18h8',
  refresh: 'M20 11a8 8 0 1 0-2 6m2-11v5h-5',
  back: 'M19 12H5l6-6m-6 6 6 6',
  right: 'm9 5 7 7-7 7',
  down: 'm5 9 7 7 7-7',
  search: 'M21 21l-5-5m2-5a7 7 0 1 1-14 0 7 7 0 0 1 14 0Z',
  warning: 'M12 4 2.5 21h19L12 4Zm0 6v4m0 3v1',
  error: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18ZM9 9l6 6m0-6-6 6',
  box: 'm12 3 9 5v8l-9 5-9-5V8l9-5Zm-9 5 9 5 9-5m-9 5v8',
} as const;

export function Icon({ name, ...props }: SVGProps<SVGSVGElement> & { name: keyof typeof paths }) {
  return <svg aria-hidden="true" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" {...props}><path d={paths[name]} /></svg>;
}

export function Badge({ status }: { status: 'ok' | 'error' | 'unset' }) {
  return <span className={`badge badge-${status}`}><span className="dot" />{status === 'ok' ? 'OK' : status === 'error' ? 'Error' : 'Unset'}</span>;
}
