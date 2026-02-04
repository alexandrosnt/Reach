import * as si from 'simple-icons';

// Map os-release ID values to simple-icons slugs
const wanted = [
  ['ubuntu', 'Ubuntu'], ['debian', 'Debian'], ['fedora', 'Fedora'],
  ['centos', 'CentOS'], ['rhel', 'RedHat'], ['redhat', 'RedHat'],
  ['rocky', 'Rockylinux'], ['almalinux', 'Almalinux'],
  ['arch', 'Archlinux'], ['manjaro', 'Manjaro'],
  ['opensuse', 'Opensuse'], ['opensuse-leap', 'Opensuse'], ['opensuse-tumbleweed', 'Opensuse'],
  ['sles', 'SUSE'], ['suse', 'SUSE'],
  ['alpine', 'Alpinelinux'], ['gentoo', 'Gentoo'],
  ['kali', 'Kalilinux'], ['nixos', 'NixOS'],
  ['void', 'Voidlinux'], ['slackware', 'Slackware'],
  ['linuxmint', 'Linuxmint'], ['pop', 'PopOS'],
  ['elementary', 'Elementary'], ['zorin', 'ZorinOS'],
  ['endeavouros', 'Endeavouros'], ['garuda', 'Garudalinux'],
  ['artix', 'Artixlinux'], ['solus', 'Solus'],
  ['mx', 'MXLinux'], ['deepin', 'Deepin'],
  ['raspbian', 'Raspberrypi'], ['raspberrypi', 'Raspberrypi'],
  ['amzn', 'Amazonwebservices'], ['ol', 'Oracle'],
  ['clear-linux-os', 'Intel'],
  ['mageia', 'Mageia'],
  ['tails', 'Tails'],
  ['parrot', 'Parrotsecurity'],
  ['freebsd', 'Freebsd'], ['openbsd', 'Openbsd'], ['netbsd', 'Netbsd'],
  ['darwin', 'Apple'], ['windows', 'Windows'],
  ['linux', 'Linux'],
];

const found = new Map();
const notFoundList = [];

for (const [osId, slug] of wanted) {
  const key = 'si' + slug;
  const icon = si[key];
  if (icon) {
    if (!found.has(slug)) {
      found.set(slug, { path: icon.path, hex: icon.hex, title: icon.title, osId });
    }
  } else {
    notFoundList.push({ osId, slug, key });
  }
}

// Output the icon data as JSON
const output = {};
for (const [slug, data] of found) {
  output[slug] = { path: data.path, hex: data.hex, title: data.title };
}

console.log(JSON.stringify(output, null, 2));

if (notFoundList.length > 0) {
  console.error('\nNot found:');
  for (const item of notFoundList) {
    console.error(`  ${item.osId} -> tried key: ${item.key}`);
  }
}
