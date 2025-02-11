import { Footer, Layout, Navbar } from 'nextra-theme-docs';
import { Banner, Head } from 'nextra/components';
import { getPageMap } from 'nextra/page-map';
import 'nextra-theme-docs/style.css';

export const metadata = {
  // Define your metadata here
  // For more information on metadata API, see: https://nextjs.org/docs/app/building-your-application/optimizing/metadata
  title: {
    template: '%s - Boros',
  },
};

const banner = (
  <Banner storageKey="boros-docs">
    Boros docs released 🎉
  </Banner>
);
const navbar = (
  <Navbar
    logo={<b>Boros - Documentation</b>}
    // ... Your additional navbar options
  />
);
const footer = (
  <Footer>
    Copyright {new Date().getFullYear()} © Boros.
  </Footer>
);

export default async function RootLayout({ children }) {
  return (
    <html
      // Not required, but good for SEO
      lang="en"
      // Required to be set
      dir="ltr"
      // Suggested by `next-themes` package https://github.com/pacocoursey/next-themes#with-app
      suppressHydrationWarning
    >
      <Head
      // ... Your additional head options
      >
        {/* Your additional tags should be passed as `children` of `<Head>` element */}
      </Head>
      <body>
        <Layout
          banner={banner}
          navbar={navbar}
          sidebar={{
            defaultMenuCollapseLevel: 1,
          }}
          pageMap={await getPageMap()}
          docsRepositoryBase="https://github.com/txpipe/boros/tree/main/docs"
          footer={footer}
          // ... Your additional layout options
        >
          {children}
        </Layout>
      </body>
    </html>
  );
}
