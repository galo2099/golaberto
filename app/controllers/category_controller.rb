class CategoryController < ApplicationController
  scaffold :category
  before_filter :login_required, :except => [ :index, :list, :show ]
end
