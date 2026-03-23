import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  async rewrites() {
    return [
      {
        source: "/health",
        destination: "http://localhost:3000/health",
      },
      {
        source: "/api/:path*",
        destination: "http://localhost:3000/api/:path*",
      },
      {
        source: "/ws/:path*",
        destination: "http://localhost:3000/ws/:path*",
      },
    ];
  },
};

export default nextConfig;
