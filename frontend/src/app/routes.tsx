import { createBrowserRouter } from "react-router-dom";
import { AppLayout } from "./App";
import { NewsListPage } from "../pages/NewsList";
import { FeedsPage } from "../pages/Feeds";
import { FeaturedPage } from "../pages/Featured";
import { SearchPage } from "../pages/Search";

export const router = createBrowserRouter([
  {
    path: "/",
    element: <AppLayout />,
    children: [
      { index: true, element: <NewsListPage /> },
      { path: "featured", element: <FeaturedPage /> },
      { path: "search", element: <SearchPage /> },
      { path: "feeds", element: <FeedsPage /> },
    ],
  },
]);
