class CategoryController < ApplicationController
  N_("Category")

  scaffold :category
  before_filter :login_required, :except => [ :index, :list, :show ]
end
