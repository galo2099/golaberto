class CategoryController < ApplicationController
  N_("Category")

  scaffold :category
  require_role "editor", :except => [ :index, :list, :show ]
end
