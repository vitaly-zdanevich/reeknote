# FreeBSD Port Draft

This directory contains a draft FreeBSD port for Reeknote. It is not submitted
to the FreeBSD ports tree yet and has not been validated on FreeBSD.

To try it on a FreeBSD machine with a ports tree:

```sh
sudo mkdir -p /usr/ports/deskutils/reeknote
sudo cp -R packaging/freebsd/reeknote/. /usr/ports/deskutils/reeknote/
cd /usr/ports/deskutils/reeknote
sudo make makesum
make stage
make check-plist
make package
```

For stronger validation before submitting to Bugzilla, run:

```sh
poudriere testport -j <jail> -p <ports-tree> deskutils/reeknote
```

Regenerate the Cargo crate list after dependency changes:

```sh
python3 tools/generate_freebsd_cargo_crates.py \
  Cargo.lock \
  packaging/freebsd/reeknote/Makefile.crates
```

Before submitting upstream, make sure `DISTVERSION` matches the release tag,
run `make makesum`, and include the generated `distinfo` in the FreeBSD ports
patch.
