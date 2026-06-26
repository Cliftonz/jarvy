Name:           jarvy
Version:        VERSION_PLACEHOLDER
Release:        1%{?dist}
Summary:        Fast, cross-platform CLI that installs and manages developer tools
License:        MIT or Apache-2.0
URL:            https://github.com/Cliftonz/jarvy
Source0:        https://github.com/Cliftonz/jarvy/releases/download/v%{version}/jarvy-v%{version}-x86_64-unknown-linux-gnu.tar.gz

%description
Jarvy provisions development environments from a jarvy.toml config file.
It installs tools via native package managers (brew, apt, dnf, winget).

Features:
- 97+ supported tools across macOS, Linux, and Windows
- Parallel installation with batch package manager operations
- Post-install hooks for shell completion and configuration
- CI/CD detection for 11 providers
- Service management with Docker Compose and Tilt

%prep
%setup -q -c

%install
install -Dm755 jarvy %{buildroot}%{_bindir}/jarvy

%files
%{_bindir}/jarvy

%changelog
* %(date "+%a %b %d %Y") Jarvy Team <team@jarvy.dev> - %{version}-1
- Release %{version}
