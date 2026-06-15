PORTNAME=	reeknote
DISTVERSION=	0.8.6
CATEGORIES=	deskutils

MAINTAINER=	zdanevich.vitaly@ya.ru
COMMENT=	Command-line Evernote client
WWW=		https://github.com/vitaly-zdanevich/reeknote

LICENSE=	GPLv3
LICENSE_FILE=	${WRKSRC}/LICENSE

USES=		cargo
USE_GITHUB=	yes
GH_ACCOUNT=	vitaly-zdanevich
GH_TAGNAME=	${DISTVERSION}

CARGO_BUILD_ARGS=	--bins
CARGO_INSTALL_ARGS=	--bins
CARGO_TEST_ARGS=	--bins

PLIST_FILES=	bin/reeknote \
		bin/rnsync
PORTDOCS=	README.md

OPTIONS_DEFINE=	DOCS

post-install:
	${STRIP_CMD} ${STAGEDIR}${PREFIX}/bin/reeknote
	${STRIP_CMD} ${STAGEDIR}${PREFIX}/bin/rnsync

post-install-DOCS-on:
	${MKDIR} ${STAGEDIR}${DOCSDIR}
	${INSTALL_DATA} ${WRKSRC}/README.md ${STAGEDIR}${DOCSDIR}/README.md

.include "Makefile.crates"

.include <bsd.port.mk>
