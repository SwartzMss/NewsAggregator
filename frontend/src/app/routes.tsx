import { createBrowserRouter } from "react-router-dom";
import { AppLayout } from "./App";
import { NewsListPage } from "../pages/NewsList";
import { FeedsPage } from "../pages/Feeds";
import { FeaturedPage } from "../pages/Featured";

export const router = createBrowserRouter([
  {
    path: "/",
    element: <AppLayout />,
    children: [
      { index: true, element: <NewsListPage /> },
      { path: "featured", element: <FeaturedPage /> },
      { path: "feeds", element: <FeedsPage /> },
    ],
  },
]);
