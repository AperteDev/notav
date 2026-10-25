# Notav for Fedora, built on COPR from .copr/Makefile (the crates vendored next to the sources;
# rust-libsecret is not in Fedora yet). scripts/release.py sets the version and adds the
# changelog entry.
%bcond check 1

Name:           notav
Version:        0.1.0
Release:        1%{?dist}
Summary:        Notes and a journal kept in your Nextcloud

SourceLicense:  GPL-3.0-or-later
# The app, and the crates linked into it (%%cargo_license_summary; LICENSE.dependencies lists
# them by crate). Check it again when the dependencies change.
License:        GPL-3.0-or-later AND Apache-2.0 AND BSD-3-Clause AND CDLA-Permissive-2.0 AND ISC AND MIT AND Unicode-3.0 AND Zlib AND (0BSD OR MIT OR Apache-2.0) AND (Apache-2.0 OR ISC OR MIT) AND (Apache-2.0 OR MIT) AND (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT) AND (MIT OR Zlib OR Apache-2.0) AND (Unlicense OR MIT)
URL:            https://github.com/AperteDev/notav
Source0:        %{url}/archive/v%{version}/%{name}-%{version}.tar.gz
# cargo vendor --locked, as .copr/Makefile makes it.
Source1:        %{name}-%{version}-vendor.tar.xz

BuildRequires:  cargo-rpm-macros >= 25
BuildRequires:  gcc
BuildRequires:  make
BuildRequires:  gettext
BuildRequires:  blueprint-compiler
BuildRequires:  pkgconfig(gtk4)
BuildRequires:  pkgconfig(libadwaita-1) >= 1.8
BuildRequires:  pkgconfig(gtksourceview-5)
BuildRequires:  pkgconfig(libsecret-1)
BuildRequires:  pkgconfig(sqlite3)
BuildRequires:  glib2-devel
BuildRequires:  desktop-file-utils
BuildRequires:  appstream

# The UI files use Adw.ShortcutsDialog; the library's soname does not say which 1.x it is.
Requires:       libadwaita%{?_isa} >= 1.8
Suggests:       gnome-online-accounts

%description
Notav keeps your notes as plain Markdown files in your Nextcloud, so the
Nextcloud Notes app and Obsidian can open the same folder. It works offline,
syncs while it runs, and keeps both versions of a note edited on two devices
at once.

%prep
%autosetup -p1 -a1
%cargo_prep -v vendor

%build
%cargo_build -- -p notav-linux
%{cargo_license_summary}
%{cargo_license} > LICENSE.dependencies
%cargo_vendor_manifest

%install
make -C linux install-data PREFIX=%{_prefix} DESTDIR=%{buildroot}
%find_lang %{name}

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/dev.aperte.Notav.desktop
appstreamcli validate --no-net %{buildroot}%{_metainfodir}/dev.aperte.Notav.metainfo.xml
%if %{with check}
# Unit tests; the integration tests need a Nextcloud account.
%cargo_test -- -p notav-core -p notav-linux
%endif

%files -f %{name}.lang
%license LICENSE LICENSE.dependencies cargo-vendor.txt
%doc README.md CHANGELOG.md
%{_bindir}/notav-linux
%{_datadir}/applications/dev.aperte.Notav.desktop
%{_metainfodir}/dev.aperte.Notav.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/dev.aperte.Notav.svg
%{_datadir}/icons/hicolor/symbolic/apps/dev.aperte.Notav-symbolic.svg

%changelog
* Sun Oct 25 2026 SelfRef <git@selfref.dev> - 0.1.0-1
- See CHANGELOG.md
