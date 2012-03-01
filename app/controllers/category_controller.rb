class CategoryController < ApplicationController
  N_("Category")

  scaffold :category
  authorize_resource
end
